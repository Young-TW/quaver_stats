use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct CacheEntry {
    pub value: Vec<u8>, // 快取的資料，例如 PNG 圖片的二進位內容
    pub expires_at: Instant, // 過期時間
}

pub struct Cache {
    store: RwLock<HashMap<String, CacheEntry>>,
    ttl: Duration, // 快取的存活時間
}

impl Cache {
    pub fn new(ttl: Duration) -> Self {
        Self {
            store: RwLock::new(HashMap::new()),
            ttl,
        }
    }

    // 獲取快取
    pub async fn get(&self, key: &str) -> Option<Vec<u8>> {
        let store = self.store.read().await;
        if let Some(entry) = store.get(key)
            && entry.expires_at > Instant::now()
        {
            return Some(entry.value.clone());
        }
        None
    }

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
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
