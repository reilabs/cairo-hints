# Cairo Hints

This repository adds external hints to Cairo without modifying the compiler or the VM.

It uses protocol buffers to define messages shared between Cairo and an external RPC server. Our own code runner (`scarb hints-run`) is used to execute Cairo code with hints.

## Prerequisites

- `protoc` from [here](https://grpc.io/docs/protoc-installation/)
- `scarb` from [here](https://github.com/software-mansion/scarb/releases)
- `lambdaworks/provers/cairo` from [here](https://github.com/lambdaclass/lambdaworks/tree/fed12d674418e4f09bc843b71bc90008a85b1aed) for proving only. As of February 2024, the tested revision is `fed12d6`.


## Installation

Clone this repository and run:
```bash
cargo install --path cairo-hints --locked
```

## Usage

1. Create a new project using `scarb hints-new --lang rust <PROJECT_NAME>`
2. Define messages in a `.proto` file
3. Run `scarb hints-generate`
4. In another tab, `cd rust` and start the RPC server with the command `cargo run`
5. Run `scarb hints-run --oracle-server http://127.0.0.1:3000 --layout all_cairo`
6. Integration tests can be run with `scarb hints-test --oracle-server http://127.0.0.1:3000 --layout all_cairo`


## Example Projects
* [Basic Rust sqrt hint](https://github.com/reilabs/cairo-hints/tree/main/examples/rust_sqrt)
* [Basic JavaScript sqrt hint](https://github.com/reilabs/cairo-hints/tree/main/examples/js_sqrt)
* [Complex messages](https://github.com/reilabs/cairo-hints/tree/main/examples/rust_complex)


## Testing

To run all tests in this crate execute the following command `cargo test --workspace --no-fail-fast`.
