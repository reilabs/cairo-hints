use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::{env, fs};

use anyhow::{Context, Result};
use cairo_lang_hints_test_runner::{CompiledTestRunner, TestRunConfig};
use cairo_lang_test_plugin::TestCompilation;
use cairo_proto_serde::configuration::{Configuration, ServerConfig};
use cairo_vm::types::layout_name::LayoutName;
use clap::Parser;
use scarb_metadata::{Metadata, MetadataCommand, PackageMetadata, ScarbCommand, TargetMetadata};
use scarb_ui::args::PackagesFilter;
use scarb_utils::absolute_path;

/// Execute all unit tests of a local package.
#[derive(Parser, Clone, Debug)]
#[clap(author, version, about, long_about = None)]
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

    /// Configuration file for oracle servers.
    #[arg(long)]
    servers_config_file: Option<PathBuf>,

    #[arg(long)]
    oracle_lock: Option<PathBuf>,

    #[clap(long = "layout", default_value = "all_cairo", value_parser=validate_layout)]
    layout: String,
}

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

fn str_into_layout(value: &str) -> LayoutName {
    match value {
        "plain" => LayoutName::plain,
        "small" => LayoutName::small,
        "dex" => LayoutName::dex,
        "recursive" => LayoutName::recursive,
        "starknet" => LayoutName::starknet,
        "starknet_with_keccak" => LayoutName::starknet_with_keccak,
        "recursive_large_output" => LayoutName::recursive_large_output,
        "recursive_with_poseidon" => LayoutName::recursive_with_poseidon,
        "all_solidity" => LayoutName::all_solidity,
        "all_cairo" => LayoutName::all_cairo,
        "dynamic" => LayoutName::dynamic,
        _ => LayoutName::all_cairo,
    }
}

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
        let mut service_config: Configuration = serde_json::from_reader(reader)?;

        // Get the servers config path
        let servers_config_path = absolute_path(&package, None, "servers_config", Some(PathBuf::from("servers.json")))
            .expect("servers config path must be provided either in the Scarb.toml file in the [tool.hints] section or default to servers.json in the project root.");

        // Read and parse the servers config file
        let config_content = fs::read_to_string(&servers_config_path).with_context(|| {
            format!(
                "failed to read servers config file: {}",
                servers_config_path.display()
            )
        })?;
        let servers_config: HashMap<String, ServerConfig> = serde_json::from_str(&config_content)
            .with_context(|| {
            format!(
                "failed to parse servers config file: {}",
                servers_config_path.display()
            )
        })?;

        // Add the server_config to the Configuration
        service_config.servers_config = servers_config;

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
            runner.run(&service_config, &str_into_layout(&args.layout))?;
            println!();
        }
    }

    Ok(())
}
fn find_testable_targets(package: &PackageMetadata) -> Vec<&TargetMetadata> {
    package
        .targets
        .iter()
        .filter(|target| target.kind == "test")
        .collect()
}
