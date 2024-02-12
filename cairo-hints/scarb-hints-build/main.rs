use std::{io::Result, path::PathBuf};
use cairo_proto_build::Config;
use clap::Parser;
use scarb_metadata::{MetadataCommand, PackageMetadata};
use scarb_ui::args::PackagesFilter;

/// Execute the main function of a package.
#[derive(Parser, Clone, Debug)]
#[command(author, version)]
struct Args {
    /// Name of the package.
    #[command(flatten)]
    packages_filter: PackagesFilter,

    #[arg(long)]
    definitions: Option<PathBuf>,

    #[clap(long)]
    cairo_output: Option<PathBuf>,

    #[clap(long)]
    oracle_module: Option<String>,

    #[structopt(long)]
    oracle_lock: Option<PathBuf>,
}

fn main() -> Result<()> {
    let args: Args = Args::parse();
    let metadata = MetadataCommand::new().inherit_stderr().exec().unwrap();
    let package = args.packages_filter.match_one(&metadata).unwrap();

    let definitions = absolute_path(&package, args.definitions, "definitions", None)
        .expect("oracle.proto definitions path must be provided either as an argument (--definitions proto/oracle.proto) or in the Scarb.toml file in [tool.hints] section.");

    let includes = definitions.parent().unwrap();

    let cairo_output: PathBuf = absolute_path(&package, args.cairo_output, "cairo_output", Some(PathBuf::from("src")))
        .expect("cairo output path must be provided either as an argument (--cairo-output src) or in the Scarb.toml file in the [tool.hints] section.");

    let oracle_module = args.oracle_module.or_else(|| {
        package.tool_metadata("hints").and_then(|tool_config| {
            tool_config["oracle_module"].as_str().map(String::from)
        })
    }).unwrap_or("lib.cairo".to_string());

    let lock_output = absolute_path(&package, args.oracle_lock, "oracle_lock", Some(PathBuf::from("Oracle.lock")))
        .expect("lock path must be provided either as an argument (--oracle-lock src) or in the Scarb.toml file in the [tool.hints] section.");

    Config::new()
        .out_dir(cairo_output)
        .oracle_module(&oracle_module)
        .oracle_lock(lock_output)
        .compile_protos(
            &[&definitions], 
            &[includes]
        )?;

    println!("Done");
    Ok(())
}

fn absolute_path(package: &PackageMetadata, arg: Option<PathBuf>, config_key: &str, default: Option<PathBuf>) -> Option<PathBuf> {
    let manifest_path = package.manifest_path.clone().into_std_path_buf();
    let project_dir = manifest_path.parent().unwrap();

    let definitions = arg.or_else(|| {
        package.tool_metadata("hints").and_then(|tool_config| {
            tool_config[config_key].as_str().map(PathBuf::from)
        })
    }).or(default)?;

    if definitions.is_absolute() {
        Some(definitions)
    } else {
        Some(project_dir.join(definitions))
    }
}