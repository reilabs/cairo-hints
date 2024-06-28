# Cairo Hints

Cairo Hints is an extension to Cairo language that makes programs easier to implement and cheaper to execute. They allow supplementing programs with data that is difficult to obtain in ZK circuits.

For example, calculating a square root in circuit is difficult, but verifying the result requires only a single multiplication. Therefore, it's a good candidate to be optimized by Cairo Hints. We can offload `sqrt` calculation to an external server, and only assert that `result * result == input`.

Cairo Hints uses protocol buffers to define messages shared between Cairo and an external RPC server. Our `scarb hints-run` code runner is used to execute Cairo code with hints.

## Example

```rust
// Oracle definition using Protocol Buffers 3

message Request {
    uint64 n = 1;
}

message Response {
    uint64 n = 1;
}

service SqrtOracle {
    rpc Sqrt(Request) returns (Response);
}

// Using the oracle in Cairo code

let result = SqrtOracle::sqrt(Request { n: input });

// Constraining the result of SqrtOracle in Cairo

result.n * result.n == input
```

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
5. Run `scarb hints-run --layout all_cairo`
6. Integration tests can be run with `scarb hints-test --layout all_cairo`

## Documentation
* [Command Reference](https://github.com/reilabs/cairo-hints/tree/main/documentation/Reference.md)
* [Tutorial](https://github.com/reilabs/cairo-hints/tree/main/documentation/Tutorial.md)

## Example Projects
* [Basic Rust sqrt hint](https://github.com/reilabs/cairo-hints/tree/main/examples/rust_sqrt)
* [Basic JavaScript sqrt hint](https://github.com/reilabs/cairo-hints/tree/main/examples/js_sqrt)
* [Complex messages](https://github.com/reilabs/cairo-hints/tree/main/examples/rust_complex)


## Testing

To run all tests in this crate execute the following command `cargo test --workspace --no-fail-fast`.
