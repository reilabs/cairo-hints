use crate::new_cairo::mk_cairo;
use crate::new_js::mk_js;
use crate::new_rust::mk_rust;
use crate::{fsx, restricted_names, Lang};
use anyhow::{bail, ensure, Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use indoc::formatdoc;
use itertools::Itertools;
use scarb::core::{Config, PackageName};

pub const DEFAULT_TARGET_DIR_NAME: &str = "target";

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
    pub lang: Lang,
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
                destination `{}` already exists.
                help: use a different project name.
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
            lang: opts.lang,
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
    lang: Lang,
}

fn mk(
    MkOpts {
        path,
        name,
        version_control,
        lang,
    }: MkOpts,
    config: &Config,
) -> Result<()> {
    // Create project directory in case we are called from `new` op.
    fsx::create_dir_all(&path)?;

    let canonical_path = fsx::canonicalize_utf8(&path).unwrap_or(path);
    init_vcs(&canonical_path, version_control)?;
    write_vcs_ignore(&canonical_path, config, version_control)?;

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

                1. Start the hints server:
                    1. Rust: `cd rust; cargo run`
                    2. Javascript: `cd js; npm install; npm start`
                2. Run `scarb hints-run --oracle-server http://127.0.0.1:3000 --layout all_cairo`

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

    match lang {
        Lang::Rust => mk_rust(&canonical_path, &name, &config)?,
        Lang::Js => mk_js(&canonical_path, &name, &config)?,
    }
    mk_cairo(&canonical_path, &name, &config)?;

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
