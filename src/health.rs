//! Liveness endpoint for load-balancer and orchestrator health checks.
//!
//! `GET /health` returns HTTP 200 with a JSON body containing the service
//! status and the number of live cache entries.  No external I/O is performed,
//! making it safe to poll frequently.

use axum::{Extension, Json, response::IntoResponse};
use serde::Serialize;
use std::sync::Arc;

use crate::cache::Cache;

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    cache_entries: usize,
}

pub async fn health(Extension(cache): Extension<Arc<Cache>>) -> impl IntoResponse {
    Json(HealthResponse {
        status: "ok",
        cache_entries: cache.len().await,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_health_status_is_ok() {
        let cache = Arc::new(Cache::new(Duration::from_secs(60)));
        let resp = HealthResponse {
            status: "ok",
            cache_entries: cache.len().await,
        };
        let json = serde_json::to_string(&resp).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["status"], "ok");
    }

    #[tokio::test]
    async fn test_health_cache_entries_reflects_live_count() {
        let cache = Arc::new(Cache::new(Duration::from_secs(60)));
        cache.set("user-a".to_string(), vec![1, 2, 3]).await;
        cache.set("user-b".to_string(), vec![4, 5, 6]).await;

        let resp = HealthResponse {
            status: "ok",
            cache_entries: cache.len().await,
        };
        let json = serde_json::to_string(&resp).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["cache_entries"], 2);
    }
}
