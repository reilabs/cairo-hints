mod oracle;
mod shirts;

use alexandria_math::is_power_of_two::is_power_of_two;
use oracle::{SqrtOracle};
use shirts::{Request, Response, Size, request::Inner};

fn main() -> bool {
    is_power_of_two(0) == false;
    let r = Request { inner: Option::Some(Inner { color: Size::Large }) } ;
    let result = SqrtOracle::sqrt(r);

    result.color == Size::Large
}

#[cfg(test)]
mod tests {
    use super::{Request, Response, Size, SqrtOracle, Inner, is_power_of_two};

    #[test]
    fn sqrt_test() {
        is_power_of_two(0) == false;
        let r = Request { inner: Option::Some(Inner { color: Size::Large }) } ;
        let result = SqrtOracle::sqrt(r);

        assert!(result.color == Size::Large);
    }
}