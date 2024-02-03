# Cairo 1 Hints

This repository adds RPC hints to Cairo, without modifying the compiler or the VM.

It uses protocol buffers to define messages shared between Cairo and RPC server, code generator and hint processor implemented in this repository.

## Installation

1. Make sure you have `protoc` and `scarb` installed.
2. Clone this repo and run:
    * `cargo install --path cairo-hints --locked`

## Usage

1. Create new project using `scarb hints-new` (not implemented yet). You can use the example project in `examples/hints_poc` instead.
2. Define messages in a .proto file
3. Run `scarb hints-build path-to-project-root`
4. Start RPC server that accepts json requests
5. Run `scarb hints-run --oracle-server http://0.0.0.0:3000 --service-config src/oracle.cairo.json`
6. Run integration tests using `scarb hints-test --oracle-server http://0.0.0.0:3000 --service-config src/oracle.cairo.json`


## Example Project
[See example project](https://github.com/reilabs/cairo-hints/tree/main/examples/hints_poc)
