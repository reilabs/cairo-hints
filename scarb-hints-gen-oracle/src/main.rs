use std::io::Result;
use cairo_proto_build::Config;

fn main() -> Result<()> {
    env_logger::init();
    println!("Compiling protos");
    Config::new()
        .compile_protos(
            &["/Users/gswirski/Code/starkware/proto-poc/proto/hints.proto"], 
            &["/Users/gswirski/Code/starkware/proto-poc/proto"]
        )?;
    println!("Done");
    Ok(())
}
