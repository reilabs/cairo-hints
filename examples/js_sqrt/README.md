# Example Project

It calculates `sqrt` using an RPC server implemented in Rust.

## Prerequisites

- `protoc` from [here](https://grpc.io/docs/protoc-installation/)
- `scarb` from [here](https://github.com/software-mansion/scarb/releases)
- `lambdaworks/provers/cairo` from [here](https://github.com/lambdaclass/lambdaworks/tree/fed12d674418e4f09bc843b71bc90008a85b1aed) for proving only. As of February 2024, the tested revision is `fed12d6`.

## Usage

1. Start the hints server:
    1. Rust: `cd rust; cargo run`
    2. Javascript: `cd js; npm install; npm start`
2. Run `scarb hints-run --oracle-server http://127.0.0.1:3000 --layout all_cairo`

## Extra options

If the circuit requires built-ins, it's possible to add the flag `--layout <VALUE>`

It's possible to generate trace and memory files when running the circuit
by adding the flags `--trace-file <PATH> --memory-file <PATH>`.

If the intention is to generate and verify a proof, execute `scarb hints-run` with the flag `--proof-mode`.
The proof can be generated and verified using [`lambdaworks/provers/cairo`](https://github.com/lambdaclass/lambdaworks/tree/fed12d674418e4f09bc843b71bc90008a85b1aed).
The command to generate the proof is: `platinum-prover prove <TRACE_FILE> <MEMORY_FILE> <PROOF_FILE>`.
The command to verify a proof is: `platinum-prover verify <PROOF_FILE>`.

## Testing

The command for running tests is: `scarb hints-test --oracle-server http://127.0.0.1:3000 --layout all_cairo`

## Note

Proof generation and verification has been tested exclusively with [`lambdaworks-fed12d6`](https://github.com/lambdaclass/lambdaworks/tree/fed12d674418e4f09bc843b71bc90008a85b1aed). Other versions may generate invalid proofs.

To install the prover, execute the following commands:
```bash
    git clone https://github.com/lambdaclass/lambdaworks.git
    cd lambdaworks
    git checkout fed12d674418e4f09bc843b71bc90008a85b1aed
    cd provers/cairo
    cargo install --path . --locked --features=cli,instruments,parallel
```
