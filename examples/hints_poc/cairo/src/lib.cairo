mod oracle;

use oracle::{RequestUInt32, SqrtOracle};

fn main() -> bool {
    let x = 10;

    let request = RequestUInt32 { n: x };
    let result = SqrtOracle::sqrt(request);

    result.n == x
}

#[cfg(test)]
mod tests {
    use super::{RequestUInt32, SqrtOracle};

    #[test]
    fn sqrt_test() {
        let x = 10;
        let request = RequestUInt32 { n: x };
        let result = SqrtOracle::sqrt(request);

        assert!(result.n == x);
    }
}