extern crate prost_build;
use std::io::Result;
use std::path::PathBuf;

fn main() -> Result<()> {
    println!("cargo:rerun-if-changed=../proto");
    let mut prost_build = prost_build::Config::new();
    prost_build.type_attribute(".", "#[derive(serde::Deserialize, serde::Serialize)]");
    prost_build.out_dir(PathBuf::from(r"./src"));
    prost_build.compile_protos(&["../proto/oracle.proto"], &["../proto"])
}
