use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::{OnceCell, RwLock};

/// A single cached value together with the instant it expires.
#[derive(Clone)]
pub struct CacheEntry {
    /// The cached bytes, e.g. the binary contents of a PNG image.
    pub value: Vec<u8>, // 快取的資料，例如 PNG 圖片的二進位內容
    /// The instant at and after which this entry is considered expired.
    pub expires_at: Instant, // 過期時間
}

/// An in-memory key/value cache whose entries expire after a fixed TTL.
///
/// Keys are `String`s and values are byte vectors. The store is guarded by an
/// async [`RwLock`], so all access goes through `async` methods.
pub struct Cache {
    store: RwLock<HashMap<String, CacheEntry>>,
    /// Per-key in-flight computations, used to deduplicate concurrent misses so
    /// that a cold key is generated exactly once even under a request stampede.
    in_flight: Mutex<HashMap<String, Arc<OnceCell<Vec<u8>>>>>,
    ttl: Duration, // 快取的存活時間
}

impl Cache {
    /// Creates an empty cache whose entries expire `ttl` after they are set.
    pub fn new(ttl: Duration) -> Self {
        Self {
            store: RwLock::new(HashMap::new()),
            in_flight: Mutex::new(HashMap::new()),
            ttl,
        }
    }

    /// Returns a clone of the value stored under `key`, or `None` if the key is
    /// absent or its entry has expired.
    ///
    /// ```
    /// # use std::time::Duration;
    /// # use quaver_stats::cache::Cache;
    /// # tokio::runtime::Runtime::new().unwrap().block_on(async {
    /// let cache = Cache::new(Duration::from_secs(60));
    /// assert_eq!(cache.get("absent").await, None);
    /// cache.set("k".to_string(), vec![1, 2, 3]).await;
    /// assert_eq!(cache.get("k").await, Some(vec![1, 2, 3]));
    /// # });
    /// ```
    // 獲取快取
    pub async fn get(&self, key: &str) -> Option<Vec<u8>> {
        {
            let store = self.store.read().await;
            match store.get(key) {
                Some(entry) if entry.expires_at > Instant::now() => {
                    return Some(entry.value.clone());
                }
                None => return None,
                _ => {} // expired — fall through to evict under write lock
            }
        }

        // Evict the expired entry under a write lock.  Re-check expiry in case
        // another thread refreshed the entry between our two lock acquisitions.
        let mut store = self.store.write().await;
        match store.get(key) {
            Some(entry) if entry.expires_at > Instant::now() => Some(entry.value.clone()),
            Some(_) => {
                store.remove(key);
                None
            }
            None => None,
        }
    }

    /// Inserts `value` under `key`, overwriting any existing entry, with an
    /// expiry of now plus the cache's TTL.
    // 設定快取
    pub async fn set(&self, key: String, value: Vec<u8>) {
        let mut store = self.store.write().await;
        store.insert(
            key,
            CacheEntry {
                value,
                expires_at: Instant::now() + self.ttl,
            },
        );
    }

    /// Returns the number of entries that are currently live (not yet expired).
    pub async fn len(&self) -> usize {
        let store = self.store.read().await;
        let now = Instant::now();
        store.values().filter(|e| e.expires_at > now).count()
    }

    /// Returns the cached value for `key`, computing it with `compute` on a miss.
    ///
    /// Concurrent callers that miss on the same cold key share a single run of
    /// `compute`: exactly one executes it while the others await its result.
    /// This prevents the cache stampede of issue #11, where every concurrent
    /// request for an uncached key would otherwise generate the value
    /// independently. On completion the value is stored with the usual TTL, so
    /// later callers take the normal cache path instead of recomputing.
    ///
    /// An empty result is treated as a non-value: it is deduplicated among the
    /// in-flight callers but is **not** stored, so a later request recomputes
    /// it. Callers use this to signal a failed computation that must not be
    /// cached (e.g. an upstream error producing no card bytes).
    pub async fn get_or_compute<F, Fut>(&self, key: &str, compute: F) -> Vec<u8>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Vec<u8>>,
    {
        if let Some(value) = self.get(key).await {
            return value;
        }

        // Join an existing in-flight computation for this key, or register a new
        // shared cell that all racing callers initialise through exactly once.
        let cell = {
            let mut in_flight = self.in_flight.lock().unwrap();
            Arc::clone(in_flight.entry(key.to_string()).or_default())
        };

        let value = cell
            .get_or_init(|| async {
                let value = compute().await;
                // Only persist real results; an empty value signals a failed
                // computation that should not be cached (see the doc comment).
                if !value.is_empty() {
                    self.set(key.to_string(), value.clone()).await;
                }
                value
            })
            .await
            .clone();

        // The value now lives in the cache, so the in-flight entry is no longer
        // needed; drop it so a future cold key starts a fresh computation.
        self.in_flight.lock().unwrap().remove(key);

        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_len_empty_cache_is_zero() {
        let cache = Cache::new(Duration::from_secs(60));
        assert_eq!(cache.len().await, 0);
    }

    #[tokio::test]
    async fn test_len_counts_live_entries_only() {
        // One entry with a real TTL, one that expires immediately.
        let cache = Cache::new(Duration::from_secs(60));
        cache.set("live".to_string(), vec![1]).await;
        {
            // Insert an already-expired entry by using a zero-TTL cache and
            // sharing the backing store via a second cache instance is not
            // possible, so we rely on the write-lock path: insert, then
            // manually confirm via get (which evicts on read — but len() uses
            // an independent read so we just verify count after expiry).
            let short = Cache::new(Duration::from_millis(0));
            short.set("gone".to_string(), vec![2]).await;
            tokio::time::sleep(Duration::from_millis(5)).await;
            // The expired entry is in `short`, not in `cache`; we test `cache`
            // which holds exactly one live entry.
            assert_eq!(short.len().await, 0, "expired entry must not be counted");
        }
        assert_eq!(cache.len().await, 1);
    }

    #[tokio::test]
    async fn test_set_then_get_hits() {
        let cache = Cache::new(Duration::from_secs(60));
        cache.set("k".to_string(), vec![1, 2, 3]).await;
        assert_eq!(cache.get("k").await, Some(vec![1, 2, 3]));
    }

    #[tokio::test]
    async fn test_get_missing_key_returns_none() {
        let cache = Cache::new(Duration::from_secs(60));
        assert_eq!(cache.get("nope").await, None);
    }

    #[tokio::test]
    async fn test_entry_expires_after_ttl() {
        // TTL 為 0，存入後立即過期
        let cache = Cache::new(Duration::from_millis(0));
        cache.set("k".to_string(), vec![9]).await;
        // 確保時間已前進超過過期點
        tokio::time::sleep(Duration::from_millis(5)).await;
        assert_eq!(cache.get("k").await, None);
    }

    #[tokio::test]
    async fn test_set_overwrites_existing_value() {
        let cache = Cache::new(Duration::from_secs(60));
        cache.set("k".to_string(), vec![1]).await;
        cache.set("k".to_string(), vec![2]).await;
        assert_eq!(cache.get("k").await, Some(vec![2]));
    }

    /// Regression test for GitHub issue #11: concurrent requests for the same
    /// uncached key must NOT each trigger an independent generation.
    ///
    /// The fix introduces a per-key in-flight guard so that, when N requests
    /// race on a cold key, the expensive computation runs exactly once and all
    /// waiters receive the same result. This test models the issue's concrete
    /// example: 10 concurrent requests for one cold key. With proper in-flight
    /// deduplication the generator runs exactly once; with the bug present every
    /// request falls through the cache miss and generates independently (the
    /// counter would read 10, not 1).
    ///
    /// This targets a `Cache::get_or_compute` deduplication entry point — the
    /// guard the issue's acceptance criteria require. The production handler
    /// (`generate_card`) should route its cache-miss path through it.
    #[tokio::test]
    async fn test_concurrent_cold_key_computes_once() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};

        let cache = Arc::new(Cache::new(Duration::from_secs(60)));
        let calls = Arc::new(AtomicUsize::new(0));

        let mut handles = Vec::new();
        for _ in 0..10 {
            let cache = Arc::clone(&cache);
            let calls = Arc::clone(&calls);
            handles.push(tokio::spawn(async move {
                cache
                    .get_or_compute("hot-user", move || async move {
                        // Stand in for the expensive generate_card_image work.
                        // The sleep keeps the winner "in flight" long enough for
                        // the other 9 requests to arrive and race the cold key.
                        calls.fetch_add(1, Ordering::SeqCst);
                        tokio::time::sleep(Duration::from_millis(50)).await;
                        vec![1, 2, 3]
                    })
                    .await
            }));
        }

        let mut results = Vec::new();
        for handle in handles {
            results.push(handle.await.expect("spawned task should not panic"));
        }

        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "10 concurrent requests for one cold key must compute exactly once (issue #11)"
        );
        for result in &results {
            assert_eq!(
                result,
                &vec![1, 2, 3],
                "every concurrent waiter must receive the deduplicated result"
            );
        }

        // After the in-flight request completes the value is cached, so a later
        // request takes the normal cache path rather than recomputing.
        assert_eq!(cache.get("hot-user").await, Some(vec![1, 2, 3]));
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "a post-completion request must hit the cache, not recompute"
        );
    }

    /// Regression test for GitHub issue #4: expired entries must be evicted
    /// from the backing `HashMap`, not merely skipped by `get`.
    ///
    /// Bug: `Cache::get` checks `expires_at > Instant::now()` and returns
    /// `None` for stale entries, but never removes them. The `HashMap`
    /// therefore grows monotonically — every unique username ever requested
    /// occupies a `CacheEntry` (with a full PNG blob) indefinitely, causing
    /// OOM under sustained load.
    ///
    /// Expected (correct) behaviour: after TTL expiry, the backing store must
    /// be empty (`store.len() == 0`), not merely returning `None` from `get`.
    #[tokio::test]
    async fn test_expired_entries_are_evicted_from_backing_store() {
        let cache = Cache::new(Duration::from_millis(0));

        // Populate 1 000 unique keys, mirroring the soak scenario in the issue.
        for i in 0..1_000usize {
            cache.set(format!("user-{i}"), vec![i as u8]).await;
        }

        // Ensure every entry has passed its TTL.
        tokio::time::sleep(Duration::from_millis(5)).await;

        // Access (miss) every key so any lazy-eviction path has a chance to run.
        for i in 0..1_000usize {
            assert_eq!(
                cache.get(&format!("user-{i}")).await,
                None,
                "get must return None for expired key user-{i}"
            );
        }

        // The critical assertion from the issue: the store itself must be empty.
        // A non-zero length here proves the memory leak is present.
        let store_len = cache.store.read().await.len();
        assert_eq!(
            store_len, 0,
            "backing HashMap must be empty after all entries expire; \
             found {store_len} stale entries — they are never evicted (issue #4)"
        );
    }
}
