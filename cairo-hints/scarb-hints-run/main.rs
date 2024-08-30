use std::{
    collections::HashMap,
    env,
    fs::{self, File},
    io::BufReader,
    path::PathBuf,
};

use anyhow::{Context, Result};
use args::process_args;
use cairo_lang_sierra::program::VersionedProgram;
use cairo_oracle_hint_processor::{run_1, Error, FuncArgs};
use cairo_proto_serde::configuration::{Configuration, ServerConfig};
use cairo_vm::types::layout_name::LayoutName;
use camino::Utf8PathBuf;
use clap::Parser;
use itertools::Itertools;
use scarb_metadata::{MetadataCommand, ScarbCommand};
use scarb_ui::args::PackagesFilter;
use scarb_utils::absolute_path;

pub mod args;
mod deserialization;

/// Execute the main function of a package.
#[derive(Parser, Clone, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Name of the package.
    #[clap(flatten)]
    packages_filter: PackagesFilter,

    /// Do not rebuild the package.
    #[clap(long, default_value_t = false)]
    no_build: bool,

    #[clap(long = "layout", default_value = "all_cairo", value_parser=validate_layout)]
    layout: String,

    #[clap(long, default_value_t = false)]
    proof_mode: bool,

    #[clap(
        long = "cairo_pie_output",
        conflicts_with_all = ["proof_mode", "air_private_input", "air_public_input"]
    )]
    cairo_pie_output: Option<PathBuf>,

    #[clap(long = "air_public_input", requires = "proof_mode")]
    air_public_input: Option<PathBuf>,

    #[clap(
        long = "air_private_input",
        requires_all = ["proof_mode", "trace_file", "memory_file"] 
    )]
    air_private_input: Option<PathBuf>,

    /// Configuration file for oracle servers.
    #[clap(long)]
    servers_config_file: Option<PathBuf>,

    /// Oracle lock file path.
    #[clap(long)]
    oracle_lock: Option<PathBuf>,

    #[clap(long)]
    trace_file: Option<PathBuf>,

    #[clap(long)]
    memory_file: Option<PathBuf>,

    /// Arguments of the Cairo function.
    #[clap(long = "args", default_value = "", value_parser=process_args)]
    args: Option<FuncArgs>,

    /// Arguments of the Cairo function.
    #[clap(long = "args_json", default_value = "")]
    args_json: Option<String>,
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

fn main() -> Result<(), Error> {
    let args: Args = Args::parse();
    let metadata = MetadataCommand::new().inherit_stderr().exec().unwrap();
    let package = args.packages_filter.match_one(&metadata).unwrap();

    if !args.no_build {
        ScarbCommand::new().arg("build").run().unwrap();
    }

    let filename = format!("{}.sierra.json", package.name);
    let scarb_target_dir = env::var("SCARB_TARGET_DIR").unwrap();
    let scarb_profile = env::var("SCARB_PROFILE").unwrap();
    let path = Utf8PathBuf::from(scarb_target_dir.clone())
        .join(scarb_profile.clone())
        .join(filename.clone());

    path.try_exists()
        .expect("package has not been compiled, file does not exist: {filename}");

    let lock_output = absolute_path(&package, args.oracle_lock, "oracle_lock", Some(PathBuf::from("Oracle.lock")))
        .expect("lock path must be provided either as an argument (--oracle-lock src) or in the Scarb.toml file in the [tool.hints] section.");
    let lock_file = File::open(lock_output).map_err(|e| Error::IO(e))?;
    let reader = BufReader::new(lock_file);
    let mut service_configuration: Configuration =
        serde_json::from_reader(reader).map_err(|e| Error::IO(e.into()))?;

    // Get the servers config path using absolute_path
    let servers_config_path = absolute_path(&package, None, "servers_config", Some(PathBuf::from("servers.json")))
        .expect("servers config path must be provided either in the Scarb.toml file in the [tool.hints] section or default to servers.json in the project root.");

    // Read and parse the servers config file
    let config_content = fs::read_to_string(&servers_config_path).map_err(|e| Error::IO(e))?;
    let servers_config: HashMap<String, ServerConfig> = serde_json::from_str(&config_content)
        .map_err(|e| {
            Error::ServersConfigFileError(format!("Failed to parse servers config: {}", e))
        })?;

    // Add the servers_config to the Configuration
    service_configuration.servers_config = servers_config;

    let sierra_program = serde_json::from_str::<VersionedProgram>(
        &fs::read_to_string(path.clone())
            .with_context(|| format!("failed to read Sierra file: {path}"))
            .unwrap(),
    )
    .with_context(|| format!("failed to deserialize Sierra program: {path}"))
    .unwrap()
    .into_v1()
    .with_context(|| format!("failed to load Sierra program: {path}"))
    .unwrap();

    let sierra_program = sierra_program.program;

    let func_args = if let Some(json_args) = args.args_json {
        let inputs_shema = absolute_path(&package, None, "inputs_schema", Some(PathBuf::from("InputsSchema.txt")))
        .expect("inputs schema path must be provided either in the Scarb.toml file in the [tool.hints] section or default to InputsSchema.txt in the project root.");

        let schema =
            args::parse_input_schema(&inputs_shema).expect("Failed to parse input schema file");

        args::process_json_args(&json_args, &schema).expect("Failed to process json args.")
    } else if let Some(args) = args.args {
        args
    } else {
        FuncArgs::default()
    };

    match run_1(
        &service_configuration,
        &str_into_layout(&args.layout),
        &args.trace_file,
        &args.memory_file,
        &args.cairo_pie_output,
        &args.air_public_input,
        &args.air_private_input,
        &func_args,
        &sierra_program,
        "::main",
        args.proof_mode,
    ) {
        Err(Error::Cli(err)) => err.exit(),
        Ok(return_values) => {
            if return_values.is_some() {
                let return_values_string_list =
                    return_values.iter().map(|m| m.to_string()).join(", ");
                println!("Return values : [{}]", return_values_string_list);
            }
            Ok(())
        }
        Err(Error::RunPanic(panic_data)) => {
            if !panic_data.is_empty() {
                let panic_data_string_list = panic_data
                    .iter()
                    .map(|m| {
                        // Try to parse to utf8 string
                        let msg = String::from_utf8(m.to_bytes_be().to_vec());
                        if let Ok(msg) = msg {
                            format!("{} ('{}')", m, msg)
                        } else {
                            m.to_string()
                        }
                    })
                    .join(", ");
                println!("Run panicked with: [{}]", panic_data_string_list);
            }
            Ok(())
        }
        Err(err) => Err(err),
    }
}
