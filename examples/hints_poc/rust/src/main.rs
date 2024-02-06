use axum::{extract, routing::post, Json, Router};
use serde::{Deserialize, Serialize};
use serde_repr::*;
use tower_http::trace::TraceLayer;
use tracing::debug;

#[derive(Debug, Serialize_repr, Deserialize_repr)]
#[repr(i32)]
enum Size {
    Small,
    Medium,
    Large,
}

#[derive(Debug, Deserialize)]
struct Inner {
    color: Size,
}

#[derive(Debug, Deserialize)]
struct Request {
    inner: Option<Inner>,
}

#[derive(Debug, Serialize)]
struct JsonResult {
    result: Response,
}

#[derive(Debug, Serialize)]
struct Response {
    color: Size,
}

async fn root(extract::Json(payload): extract::Json<Request>) -> Json<JsonResult> {
    debug!("received payload {payload:?}");
    let n = payload; //(payload.n as f64).sqrt() as u64;
    Json(JsonResult {
        result: Response {
            color: n.inner.unwrap().color,
        },
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
