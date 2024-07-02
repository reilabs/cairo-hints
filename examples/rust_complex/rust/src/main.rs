mod shirts;

use axum::{
    extract::{Json, Path, State},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use shirts::*;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
#[allow(unused_imports)]
use tokio::time::{sleep, Duration};
use tower_http::trace::TraceLayer;
use tracing::debug;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
struct JsonResult {
    result: Response,
}

#[allow(non_snake_case)]
#[derive(Debug, Serialize, Deserialize)]
struct JobResponse {
    jobId: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct JobStatus {
    status: String,
    result: Option<Response>,
}

struct AppState {
    jobs: Mutex<HashMap<String, JobStatus>>,
}

async fn create_job(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<Request>,
) -> Json<JobResponse> {
    debug!("received payload {:?}", payload);
    let job_id = Uuid::new_v4().to_string();

    let mut jobs = state.jobs.lock().unwrap();
    jobs.insert(
        job_id.clone(),
        JobStatus {
            status: "processing".to_string(),
            result: None,
        },
    );

    let job_id_clone = job_id.clone();
    let state_clone = Arc::clone(&state);

    tokio::spawn(async move {
        // Uncomment this line to simulate long-running process
        // sleep(Duration::from_secs(5)).await; 
        let mut jobs = state_clone.jobs.lock().unwrap();
        if let Some(job) = jobs.get_mut(&job_id_clone) {
            job.status = "completed".to_string();
            job.result = Some(Response {
                color: payload.inner.unwrap().color,
            });
        }
    });

    Json(JobResponse { jobId: job_id })
}

async fn get_job_status(
    State(state): State<Arc<AppState>>,
    Path(job_id): Path<String>,
) -> Json<JobStatus> {
    let jobs = state.jobs.lock().unwrap();
    let status = jobs.get(&job_id).cloned().unwrap_or(JobStatus {
        status: "not_found".to_string(),
        result: None,
    });
    Json(status)
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    let state = Arc::new(AppState {
        jobs: Mutex::new(HashMap::new()),
    });

    let app = Router::new()
        .route("/shirt", post(create_job))
        .route("/status/:job_id", get(get_job_status))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .expect("Failed to bind to port 3000, port already in use by another process. Change the port or terminate the other process.");

    debug!("Server started on http://0.0.0.0:3000");
    axum::serve(listener, app).await.unwrap();
}