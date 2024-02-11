# Example Project

It calculates sqrt using an RPC server implemented in Rust.

## Usage

1. Follow [installation guide from the root folder](https://github.com/reilabs/cairo-hints/tree/main?tab=readme-ov-file#cairo-1-hints).
2. `cd cairo`
3. Run `scarb hints-build path-to-this-folder` (i.e. `scarb hints-build ..`)
4. In a new shell tab
    * `cd rust; cargo run`
5. Run `scarb hints-run --oracle-server http://127.0.0.1:3000 --service-config src/oracle.cairo.json --trace_file lib.trace --memory_file lib.memory --layout all_cairo`
6. Integration tests: `scarb hints-test --oracle-server http://127.0.0.1:3000 --service-config src/oracle.cairo.json`
