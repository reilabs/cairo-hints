Cairo Hints is an extension to Cairo ≥ 1 language that enables operations difficult to implement inside arithmetic circuits (which are limited to addition and multiplication). Thanks to hints, it’s possible to e.g. cost effectively perform operations like square roots or binary decomposition.

Cairo Hints leverages Protocol Buffers and JSON-RPC to offload computations to an external process and feed results back to the Cairo Virtual Machine.

Users can define a service like the one below in Protocol Buffers:

```protobuf
service SqrtOracle {
    rpc Sqrt(Request) returns (Response);
}
```

To generate Cairo code that will allow them to call an external process like this:

```rust
let request = Request { ... };
let result = SqrtOracle::sqrt(request);
```

### Prerequisites

If you wish to follow this tutorial on your computer, install the following tools:

- Rust: https://www.rust-lang.org/tools/install
- Protoc: https://grpc.io/docs/protoc-installation/
- Scarb: https://docs.swmansion.com/scarb/download.html
- Node/npm (for JavaScript example): https://nodejs.org/en/download

### Installation

To install Cairo Hints, open the terminal and execute the following command:

```
cargo install --git https://github.com/reilabs/cairo-hints.git --locked
```

This command will install the following tools:

- `scarb hints-generate` - generate Cairo code from service definitions
- `scarb hints-new` - create a new Cairo project that leverages hints
- `scarb hints-run` - run Cairo code with hints, create trace files
- `scarb hints-test` - run unit or integration tests

We will talk about each of them, starting with `scarb-hints-new`.

### Getting started

`scarb hints-new` generates a new Cairo Hints project. To create `hello_hints` project with JSON-RPC server implemented in Rust, run the following command:

```
scarb hints-new --lang rust hello_hints
```

If you prefer to use JavaScript instead:

```
scarb hints-new --lang js hello_hints
```

### Using Protocol Buffers

To define objects and services available via JSON-RPC interface, Cairo Hints uses Profocol Buffers. [An in-depth description of the features of protobuf is available here.](https://protobuf.dev/programming-guides/proto3/) Due to Cairo limitations, `map` and `oneof` fields and not supported.

Each time a change to `.proto` file is made, Cairo code can be generated using the command `scarb hints-generate`.

Users are free to use `.proto` files to generate corresponding definitions on the JSON-RPC side. Rust example projects uses `[build.rs](http://build.rs)` file to generate Rust definitions of Request and Result objects. By contrast, the JavaScript example project operates on untyped JSON objects and does not rely on code generation at all.

### Running Cairo projects with hints

Assuming you have generated the `hello_hints` project, open the terminal and `cd` into `hello_hints/rust` folder. To start the JSON-RPC server, enter `cargo run`. This will start a local RPC server on port :3000 that receives hint requests from your Cairo code and sends back a computed output. This template uses an `axum` library to provide a HTTP server.

With JSON-RPC server running, open another terminal window and execute `scarb hints-run --oracle-server http://127.0.0.1:3000 --layout "all_cairo"`. This command executes the `main` function of the Cairo program contained in `src/lib.cairo`.  The expected output is:

```
let the oracle decide... Inputs: Object {"n": Number(1764)}
Output: {"n":42}
Return values : [1]
```

In this example, Cairo asks JSON-RPC server to calculate the square root of 1764. Rust servers replies with a result `42`. Finally, Cairo verifies if `42` is indeed an expected output by checking `42 * 42 = 1764`. Note that this check is cheap to implement, because it only uses multiplication natively supported in arithmetic circuits.

### Unconstrained hints

Writing zero knowledge circuits requires the correct set of constraints to prevent the generation of fake proofs. [Further explanation is available in the ZK Bug Tracker Repository](https://github.com/0xPARC/zk-bug-tracker?tab=readme-ov-file#under-constrained-circuits). **What this means for us is that data coming from a hint server always needs to be verified.** In the case of our square root operation, we added a constraint by always performing `output * output == input` (i.e. checking that the output is actually the square root of the input). Failure to properly verify hints output will lead to security vulnerabilities and bugs.

### Testing your hints and constraints

Putting hints in production requires a certain level of software testing. For this reason we include the `scarb hints-test` comand which runs all the functions marked `#[test]` inside `mod tests { }`. This enables unit testing of single components and assurance that edge cases are covered.

A simple test for the `sqrt` hint can be written as follows:

```rust
#[test]
fn sqrt_test() {
    let num = 1764;

    let request = Request { n: num };
    let result = SqrtOracle::sqrt(request);

    assert!(result.n * result.n == num);
}
```

The downside of the test above is that it relies on the RPC server manually started in another process. This is useful for final integration tests, but may be too much for some unit tests.

For this reason, it’s possible to simulate an RPC server by mocking the hint trait. For example, the square root oracle can be mocked with the following code:

```rust
#[generate_trait]
impl SqrtOracleMock of SqrtOracleTrait {
    fn sqrt(arg: super::oracle::Request) -> Response {
        todo!()
    }
}
```

Therefore, to prove that the circuit fails with the wrong square root output, the following unit test can be implemented.

```rust
#[cfg(test)]
mod tests {
    use super::oracle::Response;
    use super::{Request};

    #[generate_trait]
    impl SqrtOracleMock of SqrtOracleTrait {
        fn sqrt(arg: super::oracle::Request) -> Response {
            Response { n: 10 }
        }
    }

    #[test]
    #[should_panic]
    fn sqrt_test() {
        let num = 1764;

        let request = Request { n: num };
        let result = SqrtOracleMock::sqrt(request);

        assert!(result.n * result.n == num);
    }
}
```

### Proving Cairo programs with hints

So far, we’ve only talked about Cairo as a general purpose programming language. However, Cairo was born as a programming language for zero knowledge proofs. From the program execution it’s possible to generate a proof that the computation was performed correctly using `scarb hints-run` command and `platinum-prover`.

First we install `platinum-prover` using the following commands:

```bash
git clone https://github.com/lambdaclass/lambdaworks.git
cd lambdaworks
git checkout fed12d674418e4f09bc843b71bc90008a85b1aed
cd provers/cairo
cargo install --path . --locked --features=cli,instruments,parallel
```

`platinum-prover` is a binary which is capable of generating and verifying proofs.

Before we can generate the proof, we need to obtain memory and trace files from Cairo execution. These can be generated using the following command:

```bash
scarb hints-run --oracle-server http://127.0.0.1:3000 --layout "all_cairo" --trace-file sqrt.trace --memory-file sqrt.memory --proof-mode
```

Then we use `platinum-prover` to generate and verify proofs:

```bash
# Generate sqrt.proof
platinum-prover prove sqrt.trace sqrt.memory sqrt.proof

# Verify sqrt.proof
platinum-prover verify sqrt.proof
```

**Note: The `verify` command only checks that the proof is consistent with public inputs listed in the .proof file. The public input section itself is not checked, not even what Cairo program is being proved. These things need to be checked externally.**

### Adding hints to existing projects

In some cases, you may want to add Cairo Hints to an existing project. To do so, you can skip using the command `scarb hints-new` and manually add the following section to your `Scarb.toml` file:

```toml
[tool.hints]
definitions = "proto/oracle.proto" # path to service definitions
```

You need to manually create the `proto/oracle.proto` file e.g. by copying its contents from one of the example projects.

After you added a `.proto` file (and each time you make changes to it), execute `scarb hints-generate` to generate Cairo files.

From now on, to run the project, use `scarb hints-run` instead of `scarb run` , and `scarb hints-test` instead of `scarb test`.

Finally, you need to create an RPC server which will understand requests coming from Cairo. You can use any programming language capable of running an HTTP server that will accept and return JSON objects.

If the service definition is:

```protobuf
service SqrtOracle {
    rpc Sqrt(Request) returns (Response);
}
```

then the rpc call will be `HTTP POST <oracle_server>/sqrt` (the endpoint name is the name in `rpc` line of `.proto` file.

The response from the RPC server is expected to be encapsulated in the key `result` . No other keys should be present in the response JSON.

```json
{
	"result": ...
}
```

To represent error, any object without field “result” can be returned.
