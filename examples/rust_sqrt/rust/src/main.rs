mod oracle;

use axum::{
    extract,
    routing::post,
    Router,
    Json,
};
use serde::{Serialize, Deserialize};
use tracing::debug;
use tower_http::trace::TraceLayer;
use oracle::*;

#[derive(Debug, Serialize, Deserialize)]
struct JsonResult {
    result: Response
}

async fn root(extract::Json(payload): extract::Json<Request>) -> Json<JsonResult> {
    debug!("received payload {payload:?}");
    let n = (payload.n as f64).sqrt() as u64;
    Json(JsonResult { result: Response { n } })
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    let app = Router::new()
        .route("/sqrt", post(root))
        .layer(TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .expect("Failed to bind to port 3000, port already in use by another process. Change the port or terminate the other process.");
    debug!("Server started on http://0.0.0.0:3000");
    axum::serve(listener, app).await.unwrap();
}
