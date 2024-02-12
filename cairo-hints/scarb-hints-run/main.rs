use std::{
    env,
    fs::{self, File},
    io::BufReader,
    path::PathBuf,
};

use anyhow::{Context, Result};
use cairo_lang_sierra::program::VersionedProgram;
use cairo_oracle_hint_processor::{run_1, Error};
use cairo_proto_serde::configuration::Configuration;
use camino::Utf8PathBuf;
use clap::Parser;
use itertools::Itertools;
use scarb_metadata::{MetadataCommand, ScarbCommand};
use scarb_ui::args::PackagesFilter;

mod deserialization;

/// Execute the main function of a package.
#[derive(Parser, Clone, Debug)]
#[command(author, version)]
struct Args {
    /// Name of the package.
    #[command(flatten)]
    packages_filter: PackagesFilter,

    /// Print more items in memory.
    #[arg(long, default_value_t = false)]
    print_full_memory: bool,

    /// Do not rebuild the package.
    #[arg(long, default_value_t = false)]
    no_build: bool,

    // #[clap(value_parser, value_hint=ValueHint::FilePath)]
    // filename: PathBuf,
    /// Input to the program.
    #[arg(default_value = "[]")]
    program_input: deserialization::Args,

    #[clap(long = "layout", default_value = "plain", value_parser=validate_layout)]
    layout: String,

    /// Maximum amount of gas available to the program.
    #[arg(long)]
    available_gas: Option<usize>,

    /// Oracle server URL.
    #[arg(long)]
    oracle_server: Option<String>,

    #[arg(long)]
    service_config: Option<PathBuf>,

    #[clap(long = "trace_file", value_parser)]
    trace_file: Option<PathBuf>,

    #[structopt(long = "memory_file")]
    memory_file: Option<PathBuf>,
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

fn main() -> Result<(), Error> {
    let args: Args = Args::parse();

    let metadata = MetadataCommand::new().inherit_stderr().exec().unwrap();

    let package = args.packages_filter.match_one(&metadata).unwrap();
    // println!("Package {:#?}", package);

    ScarbCommand::new().arg("build").run().unwrap();

    let filename = format!("{}.sierra.json", package.name);
    // println!("filename {:#?}", filename);
    let scarb_target_dir = env::var("SCARB_TARGET_DIR").unwrap();
    let scarb_profile = env::var("SCARB_PROFILE").unwrap();
    let path = Utf8PathBuf::from(scarb_target_dir.clone())
        .join(scarb_profile.clone())
        .join(filename.clone());

    // ensure!(
    //     path.exists(),
    //     formatdoc! {r#"
    //         package has not been compiled, file does not exist: {filename}
    //         help: run `scarb build` to compile the package
    //     "#}
    // );

    let service_configuration = match args.service_config {
        Some(path) => {
            let file = File::open(path).unwrap();
            let reader = BufReader::new(file);
            serde_json::from_reader(reader).unwrap()
        }
        None => Configuration::default(),
    };

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

    match run_1(
        &service_configuration,
        &args.oracle_server,
        &args.layout,
        &args.trace_file,
        &args.memory_file,
        &sierra_program,
        "::main",
    ) {
        Err(Error::Cli(err)) => err.exit(),
        Ok(return_values) => {
            if !return_values.is_empty() {
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
                        let msg = String::from_utf8(m.to_be_bytes().to_vec());
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
