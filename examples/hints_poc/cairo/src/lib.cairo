mod oracle;

use oracle::{RequestUInt32, SqrtOracle};

fn main() -> bool {
    let x = 9223372036854775807;

    let request = RequestUInt32 { n: x*x };
    let result = SqrtOracle::sqrt(request);

    result.n == x
}

#[cfg(test)]
mod tests {
    use super::{RequestUInt32, SqrtOracle};

    #[test]
    fn sqrt_test() {
        let x: u64= 9223372036854775807;
        let request = RequestUInt32 { n: x * x };
        let result = SqrtOracle::sqrt(request);
        println!("Result {}", x);
        assert!(result.n == x);
    }
}