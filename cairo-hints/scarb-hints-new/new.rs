use crate::{fsx, restricted_names};
use anyhow::{bail, ensure, Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use indoc::{formatdoc, indoc};
use itertools::Itertools;
use once_cell::sync::Lazy;
use scarb::core::{Config, PackageName};
use scarb::ops;

pub const DEFAULT_TARGET_DIR_NAME: &str = "target";
pub const CAIRO_SOURCE_PATH: Lazy<Utf8PathBuf> =
    Lazy::new(|| ["cairo", "src", "lib.cairo"].iter().collect());
pub const CAIRO_MANIFEST_PATH: Lazy<Utf8PathBuf> =
    Lazy::new(|| ["cairo", "Scarb.toml"].iter().collect());
pub const PROTO_SOURCE_PATH: Lazy<Utf8PathBuf> =
    Lazy::new(|| ["proto", "oracle.proto"].iter().collect());
pub const SERVER_SOURCE_PATH: Lazy<Utf8PathBuf> =
    Lazy::new(|| ["rust", "src", "main.rs"].iter().collect());
pub const SERVER_MANIFEST_PATH: Lazy<Utf8PathBuf> =
    Lazy::new(|| ["rust", "Cargo.toml"].iter().collect());

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum VersionControl {
    Git,
    NoVcs,
}

#[derive(Debug)]
pub struct InitOptions {
    pub path: Utf8PathBuf,
    pub name: Option<PackageName>,
    pub vcs: VersionControl,
}

#[derive(Debug)]
pub struct NewResult {
    pub name: PackageName,
}

pub fn new_package(opts: InitOptions, config: &Config) -> Result<NewResult> {
    ensure!(
        !opts.path.exists(),
        formatdoc!(
            r#"
                destination `{}` already exists
                help: use `scarb init` to initialize the directory
            "#,
            opts.path
        )
    );

    let name = infer_name(opts.name, &opts.path, config)?;

    mk(
        MkOpts {
            path: opts.path.clone(),
            name: name.clone(),
            version_control: opts.vcs,
        },
        config,
    )
    .with_context(|| format!("failed to create package `{name}` at: {}", opts.path))?;

    Ok(NewResult { name })
}

fn infer_name(name: Option<PackageName>, path: &Utf8Path, config: &Config) -> Result<PackageName> {
    let name = if let Some(name) = name {
        name
    } else {
        let Some(file_name) = path.file_name() else {
            bail!(formatdoc! {r#"
                cannot infer package name from path: {path}
                help: use --name to override
            "#});
        };
        PackageName::try_new(file_name)?
    };

    if restricted_names::is_internal(name.as_str()) {
        config.ui().warn(formatdoc! {r#"
            the name `{name}` is a Scarb internal package, \
            it is recommended to use a different name to avoid problems
        "#});
    }

    if restricted_names::is_windows_restricted(name.as_str()) {
        if cfg!(windows) {
            bail!("cannot use name `{name}`, it is a Windows reserved filename");
        } else {
            config.ui().warn(formatdoc! {r#"
                the name `{name}` is a Windows reserved filename, \
                this package will not work on Windows platforms
            "#})
        }
    }

    Ok(name)
}

struct MkOpts {
    path: Utf8PathBuf,
    name: PackageName,
    version_control: VersionControl,
}

fn mk(
    MkOpts {
        path,
        name,
        version_control,
    }: MkOpts,
    config: &Config,
) -> Result<()> {
    // Create project directory in case we are called from `new` op.
    fsx::create_dir_all(&path)?;

    let canonical_path = fsx::canonicalize_utf8(&path).unwrap_or(path);
    init_vcs(&canonical_path, version_control)?;
    write_vcs_ignore(&canonical_path, config, version_control)?;

    // Create the `Scarb.toml` file.
    let manifest_path = canonical_path.join(CAIRO_MANIFEST_PATH.as_path());
    if !manifest_path.exists() {
        fsx::create_dir_all(manifest_path.parent().unwrap())?;

        fsx::write(
            &manifest_path,
            formatdoc! {r#"
            [package]
            name = "{name}"
            version = "0.1.0"
            edition = "2023_10"

            # See more keys and their definitions at https://docs.swmansion.com/scarb/docs/reference/manifest.html

            [dependencies]

            [tool.hints]
            definitions = "../proto/oracle.proto"  # must be provided
        "#},
        )?;
    }

    // Create the `lib.cairo` file.
    let source_path = canonical_path.join(CAIRO_SOURCE_PATH.as_path());
    if !source_path.exists() {
        fsx::create_dir_all(source_path.parent().unwrap())?;

        fsx::write(
            source_path,
            indoc! {r#"
                mod oracle;

                use oracle::{Request, SqrtOracle};

                fn main() -> bool {
                    let num = 1764;

                    let request = Request { n: num };
                    let result = SqrtOracle::sqrt(request);

                    result.n * result.n == num
                }
            "#},
        )?;
    }

    // Create the `oracle.proto` file.
    let proto_path = canonical_path.join(PROTO_SOURCE_PATH.as_path());
    if !proto_path.exists() {
        fsx::create_dir_all(proto_path.parent().unwrap())?;

        fsx::write(
            proto_path,
            indoc! {r#"
                syntax = "proto3";

                package oracle;

                message Request {
                    uint64 n = 1;
                }

                message Response {
                    uint64 n = 1;
                }

                service SqrtOracle {
                    rpc Sqrt(Request) returns (Response);
                }
            "#},
        )?;
    }

    // Create the `main.rs` file.
    let server_path = canonical_path.join(SERVER_SOURCE_PATH.as_path());
    if !server_path.exists() {
        fsx::create_dir_all(server_path.parent().unwrap())?;

        fsx::write(
            server_path,
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
            "#},
        )?;
    }

    // Create the `cargo.toml` file.
    let server_manifest_path = canonical_path.join(SERVER_MANIFEST_PATH.as_path());
    if !server_manifest_path.exists() {
        fsx::create_dir_all(server_manifest_path.parent().unwrap())?;

        fsx::write(
            &server_manifest_path,
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
            "#},
        )?;
    }

    let readme_path = canonical_path.join("README.md");
    if !readme_path.exists() {
        fsx::create_dir_all(readme_path.parent().unwrap())?;

        fsx::write(
            &readme_path,
            formatdoc! {r#"
                # Example Project

                It calculates `sqrt` using an RPC server implemented in Rust.

                ## Usage

                1. Follow [installation guide from cairo-hints](https://github.com/reilabs/cairo-hints/tree/main?tab=readme-ov-file#cairo-1-hints).
                2. `cd {name}/cairo`
                3. Run `scarb hints-build`
                4. In a new shell tab
                    * `cd {name}/rust; cargo run`
                5. Run `scarb hints-run --oracle-server http://127.0.0.1:3000 --trace_file lib.trace --memory_file lib.memory --layout all_cairo`

                ## Testing

                The command for running tests is: `scarb hints-test --oracle-server http://127.0.0.1:3000 --layout all_cairo`
            "#},
        )?;
    }

    if let Err(err) = ops::read_workspace(&manifest_path, config) {
        config.ui().warn(formatdoc! {r#"
            compiling this new package may not work due to invalid workspace configuration

            {err:?}
        "#})
    }

    Ok(())
}

fn init_vcs(_path: &Utf8Path, vcs: VersionControl) -> Result<()> {
    match vcs {
        VersionControl::Git => {
            todo!()
            // if !path.join(".git").exists() {
            //     gix::init(path)?;
            // }
        }
        VersionControl::NoVcs => {}
    }

    Ok(())
}

/// Write VCS ignore file.
fn write_vcs_ignore(path: &Utf8Path, config: &Config, vcs: VersionControl) -> Result<()> {
    let patterns = vec![DEFAULT_TARGET_DIR_NAME];

    let fp_ignore = match vcs {
        VersionControl::Git => path.join(".gitignore"),
        VersionControl::NoVcs => return Ok(()),
    };

    if !fp_ignore.exists() {
        let ignore = patterns.join("\n") + "\n";
        fsx::write(&fp_ignore, ignore)?;
    } else {
        let lines = patterns
            .into_iter()
            .map(|pat| format!("    {pat}"))
            .join("\n");
        config
            .ui()
            .warn(formatdoc! {r#"
                file `{fp_ignore}` already exists in this directory, ensure following patterns are ignored:

                {lines}
            "#});
    }

    Ok(())
}
