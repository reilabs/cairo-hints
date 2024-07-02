use crate::fsx;
use anyhow::Result;
use camino::Utf8PathBuf;
use indoc::{formatdoc, indoc};
use once_cell::sync::Lazy;
use scarb::core::{Config, PackageName};

pub const SERVER_SOURCE_PATH: Lazy<Utf8PathBuf> =
    Lazy::new(|| ["rust", "src", "main.rs"].iter().collect());
pub const SERVER_BUILD_PATH: Lazy<Utf8PathBuf> =
    Lazy::new(|| ["rust", "build.rs"].iter().collect());
pub const SERVER_MANIFEST_PATH: Lazy<Utf8PathBuf> =
    Lazy::new(|| ["rust", "Cargo.toml"].iter().collect());
pub const GITIGNORE_PATH: Lazy<Utf8PathBuf> =
    Lazy::new(|| [".gitignore"].iter().collect());


pub fn mk_rust(canonical_path: &Utf8PathBuf, name: &PackageName, _config: &Config) -> Result<()> {
    // Create the `main.rs` file.
    let filename = canonical_path.join(SERVER_SOURCE_PATH.as_path());
    if !filename.exists() {
        fsx::create_dir_all(filename.parent().unwrap())?;

        fsx::write(
            filename,
            indoc! {r#"
                mod oracle;

                use axum::{
                    extract,
                    routing::{post, get},
                    Router,
                    Json,
                };
                use serde::{Serialize, Deserialize};
                use tracing::debug;
                use tower_http::trace::TraceLayer;
                use oracle::*;
                use std::sync::{Arc, Mutex};
                use std::collections::HashMap;
                use uuid::Uuid;
                use tokio::time::{sleep, Duration};

                #[derive(Debug, Serialize, Deserialize)]
                struct JsonResult {
                    result: Response
                }

                #[derive(Debug, Serialize, Deserialize)]
                struct JobResponse {
                    job_id: String,
                }

                #[derive(Debug, Serialize, Deserialize)]
                struct JobStatus {
                    status: String,
                    result: Option<Response>,
                }

                struct AppState {
                    jobs: Mutex<HashMap<String, JobStatus>>,
                }

                async fn create_job(
                    extract::Json(payload): extract::Json<Request>,
                    state: extract::State<Arc<AppState>>,
                ) -> Json<JobResponse> {
                    debug!("received payload {payload:?}");
                    let job_id = Uuid::new_v4().to_string();

                    let mut jobs = state.jobs.lock().unwrap();
                    jobs.insert(job_id.clone(), JobStatus {
                        status: "processing".to_string(),
                        result: None,
                    });

                    let job_id_clone = job_id.clone();
                    let state_clone = Arc::clone(&state);

                    tokio::spawn(async move {
                        sleep(Duration::from_secs(5)).await; // Simulate long-running process
                        let n = (payload.n as f64).sqrt() as u64;
                        let mut jobs = state_clone.jobs.lock().unwrap();
                        if let Some(job) = jobs.get_mut(&job_id_clone) {
                            job.status = "completed".to_string();
                            job.result = Some(Response { n });
                        }
                    });

                    Json(JobResponse { job_id })
                }

                async fn get_job_status(
                    extract::Path(job_id): extract::Path<String>,
                    state: extract::State<Arc<AppState>>,
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
                        .route("/sqrt", post(create_job))
                        .route("/status/:job_id", get(get_job_status))
                        .layer(TraceLayer::new_for_http())
                        .with_state(state);

                    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
                        .await
                        .expect("Failed to bind to port 3000, port already in use by another process. Change the port or terminate the other process.");
                    debug!("Server started on http://0.0.0.0:3000");
                    axum::serve(listener, app).await.unwrap();
                }
            "#},
        )?;
    }

    // Create the `build.rs` file.
    let filename = canonical_path.join(SERVER_BUILD_PATH.as_path());
    if !filename.exists() {
        fsx::create_dir_all(filename.parent().unwrap())?;

        fsx::write(
            filename,
            indoc! {r##"
                extern crate prost_build;
                use std::io::Result;
                use std::path::PathBuf;

                fn main() -> Result<()> {
                    println!("cargo:rerun-if-changed=../proto");
                    let mut prost_build = prost_build::Config::new();
                    prost_build.type_attribute(".", "#[derive(serde::Deserialize, serde::Serialize)]");
                    prost_build.out_dir(PathBuf::from(r"./src"));
                    prost_build.compile_protos(&["../proto/oracle.proto"], &["../proto"])
                }
            "##},
        )?;
    }

    // Create the `cargo.toml` file.
    let filename = canonical_path.join(SERVER_MANIFEST_PATH.as_path());
    if !filename.exists() {
        fsx::create_dir_all(filename.parent().unwrap())?;

        fsx::write(
            &filename,
            formatdoc! {r#"
                [package]
                name = "{name}-rpc-server"
                version = "0.1.0"
                edition = "2021"

                [dependencies]
                axum = "0.7.3"
                serde = {{ version = "1.0.195", features = ["serde_derive"] }}
                serde_repr = "0.1.18"
                tokio = "1.35.1"
                tower-http = {{ version = "0.5.0", features = ["trace"] }}
                tracing = "0.1.40"
                tracing-subscriber = "0.3.18"
                prost = "0.12.3"
                uuid = {{ version = "1.3", features = ["v4"] }}

                [build-dependencies]
                prost-build = "0.12.3"
            "#},
        )?;
    }

    // Create the `.gitignore` file.
    let filename = canonical_path.join(GITIGNORE_PATH.as_path());
    if !filename.exists() {
        fsx::create_dir_all(filename.parent().unwrap())?;

        fsx::write(
            filename,
            indoc! {r#"
                target
            "#},
        )?;
    }
    
    Ok(())
}
