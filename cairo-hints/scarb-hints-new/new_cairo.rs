use crate::fsx;
use anyhow::Result;
use camino::Utf8PathBuf;
use indoc::{formatdoc, indoc};
use once_cell::sync::Lazy;
use scarb::core::{Config, PackageName};
use scarb::ops;

const CAIRO_SOURCE_PATH: Lazy<Utf8PathBuf> = Lazy::new(|| ["src", "lib.cairo"].iter().collect());
const ORACLE_SOURCE_PATH: Lazy<Utf8PathBuf> =
    Lazy::new(|| ["src", "oracle.cairo"].iter().collect());
const CAIRO_MANIFEST_PATH: Lazy<Utf8PathBuf> = Lazy::new(|| ["Scarb.toml"].iter().collect());
const PROTO_SOURCE_PATH: Lazy<Utf8PathBuf> =
    Lazy::new(|| ["proto", "oracle.proto"].iter().collect());
const ORACLE_LOCK_PATH: Lazy<Utf8PathBuf> = Lazy::new(|| ["Oracle.lock"].iter().collect());

pub(crate) fn mk_cairo(
    canonical_path: &Utf8PathBuf,
    name: &PackageName,
    config: &Config,
) -> Result<()> {
    // Create the `Scarb.toml` file.
    let manifest_path = canonical_path.join(CAIRO_MANIFEST_PATH.as_path());
    if !manifest_path.exists() {
        fsx::create_dir_all(manifest_path.parent().unwrap())?;

        fsx::write(
            &manifest_path,
            formatdoc! {r#"
            [package]
            name = "{name}"
            version = "0.1.0"
            edition = "2023_10"

            # See more keys and their definitions at https://docs.swmansion.com/scarb/docs/reference/manifest.html

            [dependencies]

            [tool.hints]
            definitions = "proto/oracle.proto"  # required
            # cairo_output = "src"
            # oracle_lock = "Oracle.lock"

        "#},
        )?;
    }

    // Create the `lib.cairo` file.
    let filename = canonical_path.join(CAIRO_SOURCE_PATH.as_path());
    if !filename.exists() {
        fsx::create_dir_all(filename.parent().unwrap())?;

        fsx::write(
            filename,
            indoc! {r#"
                mod oracle;

                use oracle::{Request, SqrtOracle};

                fn main() -> bool {
                    let num = 1764;

                    let request = Request { n: num };
                    let result = SqrtOracle::sqrt(request);

                    result.n * result.n == num
                }
            "#},
        )?;
    }

    // Create the `oracle.cairo` file.
    let filename = canonical_path.join(ORACLE_SOURCE_PATH.as_path());
    if !filename.exists() {
        fsx::create_dir_all(filename.parent().unwrap())?;

        fsx::write(
            filename,
            indoc! {r#"
                use starknet::testing::cheatcode;
                #[derive(Drop, Serde)]
                struct Request {
                    n: u64,
                }
                #[derive(Drop, Serde)]
                struct Response {
                    n: u64,
                }
                #[generate_trait]
                impl SqrtOracle of SqrtOracleTrait {
                    fn sqrt(arg: super::oracle::Request) -> super::oracle::Response {
                        let mut serialized = ArrayTrait::new();
                        arg.serialize(ref serialized);
                        let mut result = cheatcode::<'sqrt'>(serialized.span());
                        Serde::deserialize(ref result).unwrap()
                    }
                }
            "#},
        )?;
    }

    // Create the `oracle.proto` file.
    let filename = canonical_path.join(PROTO_SOURCE_PATH.as_path());
    if !filename.exists() {
        fsx::create_dir_all(filename.parent().unwrap())?;

        fsx::write(
            filename,
            indoc! {r#"
                syntax = "proto3";

                package oracle;

                message Request {
                    uint64 n = 1;
                }

                message Response {
                    uint64 n = 1;
                }

                service SqrtOracle {
                    rpc Sqrt(Request) returns (Response);
                }
            "#},
        )?;
    }

    // Create the `Oracle.lock` file.
    let filename = canonical_path.join(ORACLE_LOCK_PATH.as_path());
    if !filename.exists() {
        fsx::create_dir_all(filename.parent().unwrap())?;

        fsx::write(
            &filename,
            indoc! {r#"
                {"enums":{},"messages":{"oracle::Request":[{"name":"n","ty":{"primitive":"u64"}}],"oracle::Response":[{"name":"n","ty":{"primitive":"u64"}}]},"services":{"SqrtOracle":{"sqrt":{"input":{"message":"oracle::Request"},"output":{"message":"oracle::Response"}}}}}
            "#},
        )?;
    }

    if let Err(err) = ops::read_workspace(&manifest_path, config) {
        config.ui().warn(formatdoc! {r#"
            compiling this new package may not work due to invalid workspace configuration

            {err:?}
        "#})
    }

    Ok(())
}
