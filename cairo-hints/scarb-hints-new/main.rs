use anyhow::{Error, Result};
use camino::Utf8PathBuf;
use clap::Parser;
use new::{new_package, InitOptions, VersionControl};
use scarb::core::{Config, PackageName};
use scarb::ops::{self};

mod fsx;
mod new;
mod restricted_names;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(value_parser)]
    path: Utf8PathBuf,
    #[clap(long = "name", value_parser)]
    name: Option<PackageName>,
    #[clap(long = "lang", value_parser=validate_lang)]
    lang: String,
}

fn validate_lang(value: &str) -> Result<String, String> {
    match value {
        "js" | "rust" => Ok(value.to_string()),
        _ => Err(format!(
            "{value} is not a supported language. Choose `rust` or `js`."
        )),
    }
}

/// Arguments accepted by the `init` command.
#[derive(Parser, Clone, Debug)]
pub struct InitArgs {
    /// Set the resulting package name, defaults to the directory name.
    #[arg(long)]
    pub name: Option<PackageName>,

    /// Do not initialize a new Git repository.
    #[arg(long)]
    pub no_vcs: bool,
}

/// Arguments accepted by the `new` command.
#[derive(Parser, Clone, Debug)]
pub struct NewArgs {
    pub path: Utf8PathBuf,
    #[command(flatten)]
    pub init: InitArgs,
}

pub fn run(args: NewArgs, config: &Config) -> Result<()> {
    let _result = new_package(
        InitOptions {
            name: args.init.name,
            path: args.path,
            // At the moment, we only support Git but ideally, we want to
            // support more VCS and allow user to explicitly specify which VCS to use.
            vcs: if args.init.no_vcs {
                VersionControl::NoVcs
            } else {
                VersionControl::Git
            },
        },
        config,
    )?;

    Ok(())
}

fn exit_with_error(err: Error) {
    println!("Encountered error {}", err);
    std::process::exit(1);
}

fn main() {
    let args: Args = Args::parse();

    let manifest_path = ops::find_manifest_path(None).unwrap();
    let config = Config::builder(manifest_path).build().unwrap();
    let new_args = NewArgs {
        path: args.path,
        init: InitArgs {
            name: args.name,
            no_vcs: true,
        },
    };

    if let Err(err) = run(new_args, &config) {
        exit_with_error(err);
    }
}
