mod card;
mod user;
mod avatar;
mod cache;

use axum::{Router, routing::get, serve, extract::Extension};
use tokio::net::TcpListener;
use std::sync::Arc;
use std::time::Duration;
use cache::Cache;

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
