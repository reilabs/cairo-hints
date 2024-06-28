# Example Project

It shows how to use `enums` and dependencies with Cairo hints.

## Usage

1. Follow [installation guide from the root folder](https://github.com/reilabs/cairo-hints/tree/main?tab=readme-ov-file#cairo-1-hints).
2. `cd cairo`
3. Run `scarb hints-generate`
4. In a new shell tab
    * `cd rust; cargo run`
5. Run `scarb hints-run --trace-file lib.trace --memory-file lib.memory --layout all_cairo`
6. Integration tests: `scarb hints-test`
