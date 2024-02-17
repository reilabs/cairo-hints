# Cairo 1 Hints

This repository adds RPC hints to Cairo, without modifying the compiler or the VM.

It uses protocol buffers to define messages shared between Cairo and RPC server, code generator and hint processor implemented in this repository.


## Prerequisites

- `protoc` from [here](https://grpc.io/docs/protoc-installation/)
- `scarb-v2.4.3` from [here](https://github.com/software-mansion/scarb/releases/tag/v2.4.3)
- `lambdaworks/provers/cairo` from [here](https://github.com/lambdaclass/lambdaworks/tree/fed12d674418e4f09bc843b71bc90008a85b1aed) for proving only. As of February 2024, the tested revision is `fed12d6`.


## Installation

Clone this repo and run:
```bash
cargo install --path cairo-hints --locked
```

## Usage

1. Create new project using `scarb hints-new --lang rust <PROJ_NAME>`. You can also use the example project in `examples/hints_poc` instead.
2. Define messages in a `.proto` file
3. Run `scarb hints-build` in the folder `cairo`
4. In another tab, `cd rust` and start the RPC server with the command `cargo run`
5. Run `scarb hints-run --oracle-server http://127.0.0.1:3000 --layout all_cairo`
6. Integration tests can be run with `scarb hints-test --oracle-server http://127.0.0.1:3000  --layout all_cairo`


## Example Project
[See example project](https://github.com/reilabs/cairo-hints/tree/main/examples/hints_poc)


## Testing

To run all tests in this crate execute the following command `cargo test --workspace --no-fail-fast`.
