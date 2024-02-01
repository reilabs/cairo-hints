use axum::{extract, routing::post, Json, Router};
use serde::{Deserialize, Serialize};
use tower_http::trace::TraceLayer;
use tracing::debug;

#[derive(Debug, Deserialize)]
struct RequestUInt32 {
    n: u32,
}

#[derive(Debug, Serialize)]
struct JsonResult {
    result: ResponseUInt32,
}

#[derive(Debug, Serialize)]
struct ResponseUInt32 {
    n: u32,
}

async fn root(extract::Json(payload): extract::Json<RequestUInt32>) -> Json<JsonResult> {
    debug!("received payload {payload:?}");
    let n = payload.n; //(payload.n as f64).sqrt() as u64;
    Json(JsonResult {
        result: ResponseUInt32 { n },
    })
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    let app = Router::new()
        .route("/", post(root))
        .layer(TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    debug!("Server started on http://0.0.0.0:3000");
    axum::serve(listener, app).await.unwrap();
}
