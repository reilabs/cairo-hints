# Cairo 1 Hints

This repository adds RPC hints to Cairo, without modifying the compiler or the VM.

It uses protocol buffers to define messages shared between Cairo and RPC server, code generator and hint processor implemented in this repository.

## Installation

1. Make sure you have `protoc` from [here](https://grpc.io/docs/protoc-installation/)
2. Make sure you have at least `scarb-v2.5.1` installed from [here](https://docs.swmansion.com/scarb/download.html)
3. Clone this repo and run:
    * `cargo install --path cairo-hints --locked`

## Usage

1. Create new project using `scarb hints-new` (not implemented yet). You can use the example project in `examples/hints_poc` instead.
2. Define messages in a .proto file
3. Run `scarb hints-build`
4. Start RPC server that accepts json requests
5. Run `scarb hints-run --oracle-server http://0.0.0.0:3000 --layout all_cairo`
6. Run integration tests using `scarb hints-test --oracle-server http://0.0.0.0:3000`


## Example Project
[See example project](https://github.com/reilabs/cairo-hints/tree/main/examples/hints_poc)
