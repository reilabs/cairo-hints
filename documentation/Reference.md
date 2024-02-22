# Reference Guide

## `scarb hints-generate`

```
scarb hints-generate --help
Execute the main function of a package

Usage: scarb-hints-generate.exe [OPTIONS]

Options:
  -p, --package <SPEC>
  -w, --workspace
      --definitions <DEFINITIONS>
      --cairo-output <CAIRO_OUTPUT>
      --oracle-module <ORACLE_MODULE>
      --oracle-lock <ORACLE_LOCK>
  -h, --help                           Print help
  -V, --version                        Print version
```

`-p, --package` is to pass which packages have the protobuf processed. The default is `*`.

`-w, --workspace` indicates if all the packages in the workspace shall be processed.

`--definitions` is to pass a `.proto` file location instead of reading from `Scarb.toml`

`--cairo-output` the path to write the `cairo` file generated. It can be define in `Scarb.toml` under `[tool.hints]`. Default is `src`.

`--oracle-module` the filename of the generated `cairo` file.

`--oracle-lock` the filename of the generated `Oracle.lock` file which contains the JSON representation of the protobuf interface. Default is `Oracle.lock`

## `scarb hints-new`

```
scarb hints-new --help
Usage: scarb-hints-new.exe [OPTIONS] --lang <LANG> <PATH>

Arguments:
  <PATH>

Options:
      --name <NAME>
      --lang <LANG>  [possible values: rust, js]
  -h, --help         Print help
  -V, --version      Print version
```

The mandatory argument `<PATH>` is the folder name of the new project.

`--lang` is the language of the template RPC server. At the moment the choice is `rust` or `js` (Javascript)

`--name` is the name of the project. Default is the name of the project folder.

## `scarb hints-run`

```
scarb hints-run --help
Execute the main function of a package

Usage: scarb-hints-run.exe [OPTIONS]

Options:
  -p, --package <SPEC>
  -w, --workspace
      --no-build
      --layout <LAYOUT>                [default: plain]
      --proof-mode
      --oracle-server <ORACLE_SERVER>
      --oracle-lock <ORACLE_LOCK>
      --trace-file <TRACE_FILE>
      --memory-file <MEMORY_FILE>
  -h, --help                           Print help
  -V, --version                        Print version
```

`-p, --package` is to pass which packages have the protobuf processed. The default is `*`.

`-w, --workspace` indicates if all the packages in the workspace shall be processed.

`--no-build` skips building the cairo program.

`--layout` defines which builtins are included when executing the cairo program. Default is `plain`.

Other choices are:

```
| "small"
| "dex"
| "starknet"
| "starknet_with_keccak"
| "recursive_large_output"
| "all_cairo"
| "all_solidity"
| "dynamic"
```

`--proof-mode` flag needed if the intention is to generate a proof with `platinum-prover`.

`--oracle-server` is the ip:port of the oracle server.

`--oracle-lock` the filename of the generated `Oracle.lock` file which contains the JSON representation of the protobuf interface. Default is `Oracle.lock`

`--trace-file` is the filepath of the trace file generated when executing `scarb hints-run`. If flag is missing, no trace file is generated. Needed if using `--proof-mode`.

`--memory-file` is the filepath of the memory file generated when executing `scarb hints-run`. If flag is missing, no memory file is generated. Needed if using `--proof-mode`.

## `scarb hints-test`

```
scarb hints-test --help
Execute all unit tests of a local package

Usage: scarb-hints-test.exe [OPTIONS]

Options:
  -p, --package <SPEC>                
  -w, --workspace                      
  -f, --filter <FILTER>                
      --include-ignored                
      --ignored                        
      --oracle-server <ORACLE_SERVER>  
      --oracle-lock <ORACLE_LOCK>
      --layout <LAYOUT>                [default: plain]
  -h, --help                           Print help
  -V, --version                        Print version
```

`scarb hints-test` doesn’t provide the option to generate the memory file or trace file. Cairo programs are tested without `--proof-mode`.

`-p, --package` is to pass which packages have the protobuf processed. The default is `*`.

`-w, --workspace` indicates if all the packages in the workspace shall be processed.

`-f, --filter` is the regex to run only tests which contain `FILTER`.

`--include-ignored` is to run both ignored and not ignored tests.

`--oracle-server` is the ip:port of the oracle server.

`--oracle-lock` the filename of the generated `Oracle.lock` file which contains the JSON representation of the protobuf interface. Default is `Oracle.lock`

`--layout` defines which builtins are included when executing the cairo program. Default is `plain`.

```
| "small"
| "dex"
| "starknet"
| "starknet_with_keccak"
| "recursive_large_output"
| "all_cairo"
| "all_solidity"
| "dynamic"
```

## `Scarb.toml` - global configuration

In addition to the existing `Scarb.toml` configuration flags described in the [official documentation](https://docs.swmansion.com/scarb/docs/reference/manifest.html), we have added new options tailored to `cairo-hints`.

```bash
[tool.hints]
definitions = "proto/oracle.proto"  # mandatory
cairo_output = "src"                # optional - default "src"
oracle_lock = "Oracle.lock"         # optional - default "Oracle.lock"

```

The variable `definition` indicates the path of the `proto` file which is used by `scarb-hints-generate` to autogenerate Cairo code for the hint structs.

`cairo_output` indicates the folder used by `scarb-hints-generate` to save the autogenerated cairo code.

`oracle_lock` indicates the filename which `scarb-hints-generate` is going to use to save the JSON configuration of the hints. The configuration JSON file is needed by `scarb-hints-run` and `scarb-hints-test` to understand how to serialise and deserialise the data shared with the RPC server.
