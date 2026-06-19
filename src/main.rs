//! Binary entry point for the Quaver stats card server.
//!
//! Starts an Axum HTTP server on `0.0.0.0:3001` that serves player stats cards
//! at `GET /card/{name}`, backed by a 10-minute in-memory response cache.

mod avatar;
mod cache;
mod card;
mod user;

use axum::{Router, extract::Extension, routing::get, serve};
use cache::Cache;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    // 啟動時驗證必要資源是否存在，缺少時立即以非零狀態結束，
    // 避免每次請求才 panic（GitHub issue #6）。
    if let Err(err) = card::validate_assets() {
        eprintln!("error: {err}");
        std::process::exit(1);
    }

    // 初始化快取，設定 TTL 為 10 分鐘
    let cache = Arc::new(Cache::new(Duration::from_secs(600)));

    let app = Router::new()
        .route("/card/{name}", get(card::generate_card))
        .layer(Extension(cache)); // 傳遞快取到路由

    let listener = TcpListener::bind("0.0.0.0:3001").await.unwrap();

    println!("Listening on http://{}", listener.local_addr().unwrap());

    serve(listener, app).await.unwrap();
}
