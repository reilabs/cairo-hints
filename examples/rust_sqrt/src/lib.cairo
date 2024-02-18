mod oracle;

use oracle::{Request, SqrtOracle};

fn main() -> bool {
    let num = 1764;

    let request = Request { n: num };
    let result = SqrtOracle::sqrt(request);

    result.n * result.n == num
}
