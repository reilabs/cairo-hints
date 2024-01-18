mod oracle;

use oracle::{Request, Inner, SqrtOracle};

fn main() -> bool {
    let num = 1764;

    let request = Request { n: num, x: Option::Some(Inner { inner: 5 }), y: array![],  };
    let result = SqrtOracle::sqrt(request);

    result.n * result.n == num
}   
