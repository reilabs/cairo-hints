mod oracle;

use axum::{extract, http::StatusCode, response::IntoResponse, routing::post, Json, Router};
use oracle::*;
use serde::{Deserialize, Serialize};
use tower_http::trace::TraceLayer;
use tracing::debug;

#[derive(Debug, Serialize, Deserialize)]
struct JsonResult {
    result: Response,
}

async fn root(extract::Json(payload): extract::Json<Request>) -> impl IntoResponse {
    debug!("received payload {payload:?}");
    // Input number
    let n: u32 = payload.n;

    // Array size check
    if n.ilog2() + 1 > payload.len {
        return (StatusCode::INTERNAL_SERVER_ERROR, "Not enough bits.").into_response();
    }

    // Output array initialised to 0 with the expected size `payload.len`
    let mut n_bin = vec![0; usize::try_from(payload.len).unwrap()];

    // Perform the conversion from decimal to binary with least significant bit first.
    let mut lt_div = n;
    let mut idx = 0;
    while lt_div > 0 {
        let rem = lt_div % 2;
        let _ = std::mem::replace(&mut n_bin[idx], rem);
        lt_div /= 2;
        idx += 1;
    }
    let body = Json(JsonResult {
        result: Response { nb: n_bin },
    });
    (StatusCode::OK, body).into_response()
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    let app = Router::new()
        .route("/to_binary", post(root))
        .layer(TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .expect("Failed to bind to port 3000, port already in use by another process. Change the port or terminate the other process.");

    debug!("Server started on http://0.0.0.0:3000");
    axum::serve(listener, app).await.unwrap();
}
