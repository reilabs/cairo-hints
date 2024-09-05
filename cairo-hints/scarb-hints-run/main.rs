use std::{
    collections::HashMap,
    env,
    fs::{self, File},
    io::BufReader,
    path::PathBuf,
};

use anyhow::{Context, Result};
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
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};

mod deserialization;

#[derive(Parser, Clone, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(flatten)]
    packages_filter: PackagesFilter,

    #[clap(long, default_value_t = false)]
    no_build: bool,

    #[clap(long = "layout", default_value = "all_cairo", value_parser = validate_layout)]
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

    #[clap(long)]
    servers_config_file: Option<PathBuf>,

    #[clap(long)]
    oracle_lock: Option<PathBuf>,

    #[clap(long)]
    trace_file: Option<PathBuf>,

    #[clap(long)]
    memory_file: Option<PathBuf>,

    #[clap(long = "args", default_value = "", value_parser = process_args)]
    args: Option<FuncArgs>,

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

#[derive(Serialize, Deserialize, Debug)]
struct PreprocessResponse {
    args: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct CairoRunResponse {
    result: String,
    request_id: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let result = match run().await {
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

    std::process::exit(if result["status"] == "error" { 1 } else { 0 });
}

async fn run() -> Result<String> {
    let args: Args = Args::parse();
    let metadata = MetadataCommand::new().inherit_stderr().exec()?;
    let package = args.packages_filter.match_one(&metadata)?;

    if !args.no_build {
        ScarbCommand::new().arg("build").run()?;
    }

    let filename = format!("{}.sierra.json", package.name);
    let scarb_target_dir = env::var("SCARB_TARGET_DIR").context("SCARB_TARGET_DIR not set")?;
    let scarb_profile = env::var("SCARB_PROFILE").context("SCARB_PROFILE not set")?;
    let path = Utf8PathBuf::from(scarb_target_dir)
        .join(scarb_profile)
        .join(filename);

    if !path.try_exists()? {
        anyhow::bail!(
            "Package has not been compiled, file does not exist: {}",
            path
        );
    }

    let lock_output = absolute_path(&package, args.clone().oracle_lock, "oracle_lock", Some(PathBuf::from("Oracle.lock")))
        .context("Lock path must be provided either as an argument (--oracle-lock src) or in the Scarb.toml file in the [tool.hints] section.")?;
    let lock_file = File::open(lock_output)?;
    let reader = BufReader::new(lock_file);
    let mut service_configuration: Configuration = serde_json::from_reader(reader)?;

    let servers_config_path = absolute_path(&package, None, "servers_config", Some(PathBuf::from("servers.json")))
        .context("Servers config path must be provided either in the Scarb.toml file in the [tool.hints] section or default to servers.json in the project root.")?;

    let config_content = fs::read_to_string(&servers_config_path)?;
    let mut servers_config: HashMap<String, ServerConfig> =
        serde_json::from_str(&config_content).context("Failed to parse servers config")?;

    let sierra_program = serde_json::from_str::<VersionedProgram>(&fs::read_to_string(&path)?)?
        .into_v1()
        .context("Failed to load Sierra program")?
        .program;

    let preprocess_config = servers_config.remove("preprocess");
    let postprocess_config = servers_config.remove("postprocess");

    let func_args = get_func_args(&args, &package, preprocess_config).await?;

    service_configuration.servers_config = servers_config;

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

    process_result(result, postprocess_config).await
}

async fn get_func_args(
    args: &Args,
    package: &scarb_metadata::PackageMetadata,
    preprocess_config: Option<ServerConfig>,
) -> Result<FuncArgs> {
    let inputs_schema = get_inputs_schema(package)?;
    let schema = parse_input_schema(&inputs_schema)
        .map_err(|e| anyhow::anyhow!("Failed to parse input schema: {}", e))?;

    if let Some(preprocess) = preprocess_config {
        let body: Value =
            serde_json::from_str(&args.args_json.as_ref().context("Expect --args_json")?)?;

        let url = if preprocess.server_url.ends_with("/preprocess") {
            preprocess.server_url.to_string()
        } else if preprocess.server_url.ends_with('/') {
            format!("{}preprocess", preprocess.server_url)
        } else {
            format!("{}/preprocess", preprocess.server_url)
        };

        let preprocess_result = call_server::<PreprocessResponse>(&url, body).await?.args;
        process_json_args(&preprocess_result, &schema).map_err(|e| anyhow::anyhow!(e))
    } else if let Some(json_args) = &args.args_json {
        process_json_args(json_args, &schema).map_err(|e| anyhow::anyhow!(e))
    } else if let Some(args) = &args.args {
        Ok(args.clone())
    } else {
        Ok(FuncArgs::default())
    }
}

fn get_inputs_schema(package: &scarb_metadata::PackageMetadata) -> Result<PathBuf> {
    absolute_path(package, None, "inputs_schema", Some(PathBuf::from("InputsSchema.txt")))
        .context("Inputs schema path must be provided either in the Scarb.toml file in the [tool.hints] section or default to InputsSchema.txt in the project root.")
}

async fn process_result(
    result: Result<Option<String>, Error>,
    postprocess_config: Option<ServerConfig>,
) -> Result<String> {
    match result {
        Ok(return_values) => {
            let cairo_output = return_values.unwrap_or_else(|| "Null".to_string());
            if let Some(postprocess) = postprocess_config {
                let url = if postprocess.server_url.ends_with("/postprocess") {
                    postprocess.server_url.to_string()
                } else if postprocess.server_url.ends_with('/') {
                    format!("{}postprocess", postprocess.server_url)
                } else {
                    format!("{}/postprocess", postprocess.server_url)
                };

                let body = CairoRunResponse {
                    result: cairo_output,
                    request_id: "None".to_string(),
                };

                call_server::<Value>(&url, body)
                    .await
                    .map(|v| v.to_string())
            } else {
                Ok(cairo_output)
            }
        }
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

async fn call_server<T: DeserializeOwned>(url: &str, body: impl Serialize) -> Result<T> {
    let client = reqwest::Client::new();
    let response = client.post(url).json(&body).send().await?;
    response
        .error_for_status()?
        .json()
        .await
        .map_err(Into::into)
}
