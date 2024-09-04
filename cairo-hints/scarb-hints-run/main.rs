use std::{
    collections::HashMap,
    env,
    fs::{self, File},
    io::BufReader,
    path::PathBuf,
};

use anyhow::Result;
use cairo_lang_sierra::program::VersionedProgram;
use cairo_oracle_hint_processor::{run_1, Error, FuncArgs};
use cairo_proto_serde::configuration::{Configuration, ServerConfig};
use cairo_vm::types::layout_name::LayoutName;
use camino::Utf8PathBuf;
use clap::Parser;
use scarb_hints_lib::serialization::{parse_input_schema, process_args, process_json_args};
use scarb_hints_lib::utils::absolute_path;
use scarb_metadata::{MetadataCommand, ScarbCommand};
use scarb_ui::args::PackagesFilter;
use serde_json::{json, Value};
use std::process;

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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let result = match run() {
        Ok(return_values) => {
            let parsed_data: Value = serde_json::from_str(&return_values)?;
            json!({
                "status": "success",
                "data": parsed_data
            })
        }
        Err(err) => {
            json!({
                "status": "error",
                "message": err.to_string()
            })
        }
    };

    println!("{}", serde_json::to_string(&result)?);

    if result["status"] == "error" {
        process::exit(1);
    } else {
        process::exit(0);
    }
}

fn run() -> Result<String, Box<dyn std::error::Error>> {
    let args: Args = Args::parse();
    let metadata = MetadataCommand::new().inherit_stderr().exec()?;
    let package = args.packages_filter.match_one(&metadata)?;

    if !args.no_build {
        ScarbCommand::new().arg("build").run()?;
    }

    let filename = format!("{}.sierra.json", package.name);
    let scarb_target_dir = env::var("SCARB_TARGET_DIR")?;
    let scarb_profile = env::var("SCARB_PROFILE")?;
    let path = Utf8PathBuf::from(scarb_target_dir)
        .join(scarb_profile)
        .join(filename);

    if !path.try_exists()? {
        return Err(format!(
            "Package has not been compiled, file does not exist: {}",
            path
        )
        .into());
    }

    let lock_output = absolute_path(&package, args.oracle_lock, "oracle_lock", Some(PathBuf::from("Oracle.lock")))
        .ok_or_else(|| "Lock path must be provided either as an argument (--oracle-lock src) or in the Scarb.toml file in the [tool.hints] section.")?;
    let lock_file = File::open(lock_output)?;
    let reader = BufReader::new(lock_file);
    let mut service_configuration: Configuration = serde_json::from_reader(reader)?;

    let servers_config_path = absolute_path(&package, None, "servers_config", Some(PathBuf::from("servers.json")))
        .ok_or_else(|| "Servers config path must be provided either in the Scarb.toml file in the [tool.hints] section or default to servers.json in the project root.")?;

    let config_content = fs::read_to_string(&servers_config_path)?;
    let servers_config: HashMap<String, ServerConfig> = serde_json::from_str(&config_content)
        .map_err(|e| format!("Failed to parse servers config: {}", e))?;

    service_configuration.servers_config = servers_config;

    let sierra_program = serde_json::from_str::<VersionedProgram>(&fs::read_to_string(&path)?)?
        .into_v1()
        .map_err(|_| format!("Failed to load Sierra program: {}", path))?
        .program;

    let func_args = if let Some(json_args) = args.args_json {
        let inputs_schema = absolute_path(&package, None, "inputs_schema", Some(PathBuf::from("InputsSchema.txt")))
            .ok_or_else(|| "Inputs schema path must be provided either in the Scarb.toml file in the [tool.hints] section or default to InputsSchema.txt in the project root.")?;

        let schema = parse_input_schema(&inputs_schema)?;
        process_json_args(&json_args, &schema)?
    } else if let Some(args) = args.args {
        args
    } else {
        FuncArgs::default()
    };

    let result = run_1(
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
    );

    match result {
        Ok(return_values) => Ok(return_values.unwrap_or_else(|| "Null".to_string())),
        Err(Error::RunPanic(panic_data)) => {
            let panic_data_string = if panic_data.is_empty() {
                "Null".to_string()
            } else {
                panic_data
                    .iter()
                    .map(|m| {
                        String::from_utf8(m.to_bytes_be().to_vec())
                            .map(|msg| format!("{} ('{}')", m, msg))
                            .unwrap_or_else(|_| m.to_string())
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            Ok(format!("Run panicked with: [{}]", panic_data_string))
        }
        Err(err) => Err(err.into()),
    }
}
