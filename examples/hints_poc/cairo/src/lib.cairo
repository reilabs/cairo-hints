mod oracle;

use oracle::{Request, Size, SqrtOracle};

fn main() -> bool {
    true
    //let x = 42;
    //let r = Request { n: x * x, x: Option::Some(Inner { inner: 5 }), y: array![1,2,3,4] };
    //let result = SqrtOracle::sqrt(r);

    //result.n == x
}   

#[cfg(test)]
mod tests {
    use super::{Request, Size, SqrtOracle};

    #[test]
    fn sqrt_test() {
        let s = Size::Large;
        let c = false;
        let r = Request { color: c, size: s };
        let result = SqrtOracle::sqrt(r);

        assert!(result.color == false);
        assert!(result.size == Size::Large);
    }
}