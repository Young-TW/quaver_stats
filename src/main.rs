mod card;
mod user;

use axum::{Router, routing::get, serve};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    let app = Router::new().route("/card/{name}", get(card::generate_card));

    let listener = TcpListener::bind("0.0.0.0:3000").await.unwrap();

    println!("Listening on http://{}", listener.local_addr().unwrap());

    serve(listener, app).await.unwrap();
}
