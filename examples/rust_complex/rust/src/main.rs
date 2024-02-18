mod shirts;

use axum::{extract, routing::post, Json, Router};
use serde::{Deserialize, Serialize};
use tower_http::trace::TraceLayer;
use tracing::debug;
use shirts::*;

#[derive(Debug, Serialize, Deserialize)]
struct JsonResult {
    result: Response,
}

async fn root(extract::Json(payload): extract::Json<Request>) -> Json<JsonResult> {
    debug!("received payload {payload:?}");
    Json(JsonResult {
        result: Response {
            color: payload.inner.unwrap().color,
        },
    })
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    let app = Router::new()
        .route("/shirt", post(root))
        .layer(TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .expect("Failed to bind to port 3000, port already in use by another process. Change the port or terminate the other process.");

    debug!("Server started on http://0.0.0.0:3000");
    axum::serve(listener, app).await.unwrap();
}
