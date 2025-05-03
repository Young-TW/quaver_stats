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
        if let Some(entry) = store.get(key) {
            if entry.expires_at > Instant::now() {
                return Some(entry.value.clone());
            }
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
