# Cairo 1 Hints

This repository adds RPC hints to Cairo, without modifying the compiler or the VM.

It uses protocol buffers to define messages shared between Cairo and RPC server, code generator and hint processor implemented in this repository.

## Installation

1. Make sure you have `protoc` installed.
2. Clone this repo and run:
    * cargo install --path scarb-hints-new
    * cargo install --path scarb-hints-gen-oracle
    * cargo install --path scarb-hints-run
    * cargo install --path scarb-hints-test 

## Usage

1. Create new project using `scarb hints-new` (not implemented yet). You can use the example project in `examples/hints_poc` instead.
2. Define messages in a .proto file
3. Run `scarb hints-gen-oracle`
4. Start RPC server that accepts json requests
5. Run `scarb hints-run --oracle-server http://0.0.0.0:3000` # insert your own server url`

## Example Project
[See example project](https://github.com/reilabs/cairo-hints/tree/main/examples/hints_poc)
