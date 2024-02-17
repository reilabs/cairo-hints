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

pub fn mk_rust(canonical_path: &Utf8PathBuf, name: &PackageName, _config: &Config) -> Result<()> {
    // Create the `main.rs` file.
    let filename = canonical_path.join(SERVER_SOURCE_PATH.as_path());
    if !filename.exists() {
        fsx::create_dir_all(filename.parent().unwrap())?;

        fsx::write(
            filename,
            indoc! {r#"
                use axum::{
                    extract,
                    routing::post,
                    Router,
                    Json,
                };
                use serde::{Serialize, Deserialize};
                use tracing::debug;
                use tower_http::trace::TraceLayer;

                include!("./oracle.rs");

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
                        .route("/", post(root))
                        .layer(TraceLayer::new_for_http());

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

                [build-dependencies]
                prost-build = "0.12.3"
            "#},
        )?;
    }

    let filename = canonical_path.join("README.md");
    if !filename.exists() {
        fsx::create_dir_all(filename.parent().unwrap())?;

        fsx::write(
            &filename,
            formatdoc! {r#"
                # Example Project

                It calculates `sqrt` using an RPC server implemented in Rust.

                ## Prerequisites

                - `protoc` from [here](https://grpc.io/docs/protoc-installation/)
                - `scarb-v2.4.3` from [here](https://github.com/software-mansion/scarb/releases/tag/v2.4.3)
                - `lambdaworks/provers/cairo` from [here](https://github.com/lambdaclass/lambdaworks/tree/fed12d674418e4f09bc843b71bc90008a85b1aed) for proving only. As of February 2024, the tested revision is `fed12d6`.

                ## Usage

                1. `cd cairo`
                2. In a new shell tab
                    * `cd rust; cargo run`
                3. Run `scarb hints-run --oracle-server http://127.0.0.1:3000 --layout all_cairo`

                ## Extra options

                If the circuit requires built-ins, it's possible to add the flag `--layout <VALUE>`

                It's possible to generate trace and memory files when running the circuit
                by adding the flags `--trace_file <PATH> --memory_file <PATH>`.

                The proof can be generated and verified using [`lambdaworks/provers/cairo`](https://github.com/lambdaclass/lambdaworks/tree/fed12d674418e4f09bc843b71bc90008a85b1aed).
                The command to generate the proof is: `platinum-prover prove <TRACE_FILE> <MEMORY_FILE> <PROOF_FILE>`.
                The command to verify a proof is: `platinum-prover verify <PROOF_FILE>`.

                ## Testing

                The command for running tests is: `scarb hints-test --oracle-server http://127.0.0.1:3000 --layout all_cairo`

                ## Note

                Proof generation and verification has been tested exclusively with `scarb-v2.4.3` in combination with [`lambdaworks-fed12d6`](https://github.com/lambdaclass/lambdaworks/tree/fed12d674418e4f09bc843b71bc90008a85b1aed). Other versions may generate invalid proofs.

                To install the prover, execute the following commands:
                ```bash
                    git clone https://github.com/lambdaclass/lambdaworks.git
                    cd lambdaworks
                    git reset --hard fed12d674418e4f09bc843b71bc90008a85b1aed
                    cd provers/cairo
                    cargo install --path . --locked --features=cli,instruments,parallel
                ```
            "#},
        )?;
    }

    Ok(())
}
