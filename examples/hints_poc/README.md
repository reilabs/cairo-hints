# Example Project

It calculates sqrt using an RPC server implemented in Rust.

## Usage

1. Follow [installation guide from the root folder](https://github.com/reilabs/cairo-hints/tree/main?tab=readme-ov-file#cairo-1-hints).
2. Run `scarb hints-build path-to-this-folder`
3. In a new shell tab
    * `cd rust; cargo run`
4. Run `scarb hints-run --oracle-server http://0.0.0.0:3000 --service-config src/oracle.cairo.json`
5. Integration tests: `scarb hints-test --oracle-server http://0.0.0.0:3000 --service-config src/oracle.cairo.json`
