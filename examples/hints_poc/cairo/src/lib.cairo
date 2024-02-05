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
        let c = "Hello World~~~~";
        let r = Request { color: c.clone() };
        let result = SqrtOracle::sqrt(r);

        assert!(result.color == c.clone());
    }
}