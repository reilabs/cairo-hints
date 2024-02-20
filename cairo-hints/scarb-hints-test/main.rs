use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::{env, fs};

use anyhow::{Context, Result};
use cairo_lang_hints_test_runner::{CompiledTestRunner, TestRunConfig};
use cairo_lang_test_plugin::TestCompilation;
use clap::Parser;
use scarb_metadata::{Metadata, MetadataCommand, PackageMetadata, ScarbCommand, TargetMetadata};
use scarb_ui::args::PackagesFilter;
use scarb_utils::absolute_path;

/// Execute all unit tests of a local package.
#[doc(hidden)]
#[derive(Parser, Clone, Debug)]
#[command(author, version)]
struct Args {
    #[command(flatten)]
    packages_filter: PackagesFilter,

    /// Run only tests whose name contain FILTER.
    #[arg(short, long, default_value = "")]
    filter: String,

    /// Run ignored and not ignored tests.
    #[arg(long, default_value_t = false)]
    include_ignored: bool,

    /// Run only ignored tests.
    #[arg(long, default_value_t = false)]
    ignored: bool,

    /// Oracle server URL.
    #[arg(long)]
    oracle_server: Option<String>,

    #[arg(long)]
    oracle_lock: Option<PathBuf>,

    #[clap(long = "layout", default_value = "plain", value_parser=validate_layout)]
    layout: String,
}

#[doc(hidden)]
fn validate_layout(value: &str) -> Result<String, String> {
    match value {
        "plain"
        | "small"
        | "dex"
        | "starknet"
        | "starknet_with_keccak"
        | "recursive_large_output"
        | "all_cairo"
        | "all_solidity"
        | "dynamic" => Ok(value.to_string()),
        _ => Err(format!("{value} is not a valid layout")),
    }
}

#[doc(hidden)]
fn main() -> Result<()> {
    let args: Args = Args::parse();

    let metadata = MetadataCommand::new().inherit_stderr().exec()?;

    let matched = args.packages_filter.match_many(&metadata)?;
    let filter = PackagesFilter::generate_for::<Metadata>(matched.iter());
    ScarbCommand::new()
        .arg("build")
        .arg("--test")
        .env("SCARB_PACKAGES_FILTER", filter.to_env())
        .run()?;

    let profile = env::var("SCARB_PROFILE").unwrap_or("dev".into());
    let default_target_dir = metadata.runtime_manifest.join("target");
    let target_dir = metadata
        .target_dir
        .clone()
        .unwrap_or(default_target_dir)
        .join(profile);

    for package in matched {
        println!("testing {} ...", package.name);

        let lock_output = absolute_path(&package, args.oracle_lock.clone(), "oracle_lock", Some(PathBuf::from("Oracle.lock")))
            .expect("lock path must be provided either as an argument (--oracle-lock src) or in the Scarb.toml file in the [tool.hints] section.");
        let lock_file = File::open(lock_output)?;
        let reader = BufReader::new(lock_file);
        let service_config = serde_json::from_reader(reader)?;

        for target in find_testable_targets(&package) {
            let file_path = target_dir.join(format!("{}.test.json", target.name.clone()));
            let test_compilation = serde_json::from_str::<TestCompilation>(
                &fs::read_to_string(file_path.clone())
                    .with_context(|| format!("failed to read file: {file_path}"))?,
            )
            .with_context(|| format!("failed to deserialize compiled tests file: {file_path}"))?;

            let config = TestRunConfig {
                filter: args.filter.clone(),
                include_ignored: args.include_ignored,
                ignored: args.ignored,
            };
            let runner = CompiledTestRunner::new(test_compilation, config);
            runner.run(&args.oracle_server, &service_config, &args.layout)?;
            println!();
        }
    }

    Ok(())
}

#[doc(hidden)]
fn find_testable_targets(package: &PackageMetadata) -> Vec<&TargetMetadata> {
    package
        .targets
        .iter()
        .filter(|target| target.kind == "test")
        .collect()
}
