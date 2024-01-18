use std::{io::Result, env};
use cairo_proto_build::Config;

fn main() -> Result<()> {
    env_logger::init();
    println!("Compiling protos");

    let args: Vec<String> = env::args().collect();
    let base_path = args.get(1).expect("provide path to the project");

    Config::new()
        .out_dir(format!("{base_path}/cairo/src"))
        .compile_protos(
            &[format!("{base_path}/proto/oracle.proto")], 
            &[format!("{base_path}/proto")]
        )?;

    println!("Done");
    Ok(())
}
