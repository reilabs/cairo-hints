mod oracle;
mod shirts;

use oracle::{SqrtOracle};
use shirts::{Request, Response, Size, request};

fn main() -> bool {
    true
    //let x = 42;
    //let r = Request { n: x * x, x: Option::Some(Inner { inner: 5 }), y: array![1,2,3,4] };
    //let result = SqrtOracle::sqrt(r);

    //result.n == x
}

#[cfg(test)]
mod tests {
    use super::{Request, Response, Size, SqrtOracle, request::Inner};

    #[test]
    fn sqrt_test() {
        let r = Request { inner: Option::Some(Inner { color: Size::Large }) } ;
        let result = SqrtOracle::sqrt(r);

        assert!(result.color == Size::Large);
    }
}