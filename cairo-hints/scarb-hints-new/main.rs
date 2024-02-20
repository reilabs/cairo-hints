use anyhow::{Error, Result};
use camino::Utf8PathBuf;
use clap::{Parser, ValueEnum};
use new::{new_package, InitOptions, VersionControl};
use scarb::core::{Config, PackageName};
use scarb::ops::{self};

#[doc(hidden)]
mod fsx;

#[doc(hidden)]
mod new;

#[doc(hidden)]
mod new_cairo;

#[doc(hidden)]
mod new_js;

#[doc(hidden)]
mod new_rust;

#[doc(hidden)]
mod restricted_names;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
/// Arguments accepted `scarb-hints-new` command.
struct Args {
    /// Set the folder of the package name.
    #[clap(value_parser)]
    path: Utf8PathBuf,
    /// Set the resulting package name, defaults to the directory name.
    #[clap(long = "name", value_parser)]
    name: Option<PackageName>,
    /// Set the RPC server language, Rust or JavaScript
    #[clap(long = "lang", value_enum)]
    lang: Lang,
}

#[doc(hidden)]
#[derive(ValueEnum, Clone, Copy, Debug, PartialEq)]
#[clap(rename_all = "lower")]
enum Lang {
    Rust,
    Js,
}

#[doc(hidden)]
#[derive(Parser, Clone, Debug)]
struct InitArgs {
    #[arg(long)]
    name: Option<PackageName>,
    #[arg(long)]
    no_vcs: bool,
}

/// Arguments accepted by the `new` command.
#[doc(hidden)]
#[derive(Parser, Clone, Debug)]
struct NewArgs {
    path: Utf8PathBuf,
    #[command(flatten)]
    init: InitArgs,
    lang: Lang,
}

#[doc(hidden)]
fn run(args: NewArgs, config: &Config) -> Result<()> {
    let _result = new_package(
        InitOptions {
            name: args.init.name,
            path: args.path,
            // At the moment, the default is NoVcs
            vcs: if args.init.no_vcs {
                VersionControl::NoVcs
            } else {
                VersionControl::Git
            },
            lang: args.lang,
        },
        config,
    )?;

    Ok(())
}

#[doc(hidden)]
fn exit_with_error(err: Error) {
    println!("Encountered error {}", err);
    std::process::exit(1);
}

#[doc(hidden)]
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
        lang: args.lang,
    };

    if let Err(err) = run(new_args, &config) {
        exit_with_error(err);
    }
}
