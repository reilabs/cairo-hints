mod oracle;

use oracle::{Request, SqrtOracle};

fn main() -> Array<felt252> {
    let num = 1764;

    let request = Request { n: num };
    let result = SqrtOracle::sqrt(request);

    let res_check = (result.n * result.n == num);
    let mut output: Array<felt252> = ArrayTrait::new();
    res_check.serialize(ref output);
    output
}
