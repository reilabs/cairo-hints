use axum::{
    extract,
    routing::post,
    Router,
    Json,
};
use serde::{Serialize, Deserialize};
use tracing::debug;
use tower_http::trace::TraceLayer;

#[derive(Debug, Deserialize)]
struct Request {
    n: u64,
}

#[derive(Debug, Serialize)]
struct JsonResult {
    result: Response
}

#[derive(Debug, Serialize)]
struct Response {
    n: u64,
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
        .route("/", post(root))
        .layer(TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    debug!("Server started on http://0.0.0.0:3000");
    axum::serve(listener, app).await.unwrap();
}
