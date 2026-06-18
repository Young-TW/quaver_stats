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
    // 初始化快取，設定 TTL 為 10 分鐘
    let cache = Arc::new(Cache::new(Duration::from_secs(600)));

    let app = Router::new()
        .route("/card/{name}", get(card::generate_card))
        .layer(Extension(cache)); // 傳遞快取到路由

    let listener = TcpListener::bind("0.0.0.0:3001").await.unwrap();

    println!("Listening on http://{}", listener.local_addr().unwrap());

    serve(listener, app).await.unwrap();
}
