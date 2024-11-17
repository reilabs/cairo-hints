mod oracle;
mod shirts;

use alexandria_math::is_power_of_two::is_power_of_two;
use oracle::ShirtsOracle;
use shirts::{Request, Response, Size, request::Inner};

fn main() -> Array<felt252> {
    is_power_of_two(0) == false;
    let r = Request { inner: Option::Some(Inner { color: Size::Large }) } ;
    let result = ShirtsOracle::shirt(r);

    let res_check = (result.color == Size::Large);
    let mut output: Array<felt252> = ArrayTrait::new();
    res_check.serialize(ref output);
    output
}

#[cfg(test)]
mod tests {
    use super::{Request, Response, Size, ShirtsOracle, Inner, is_power_of_two};

    #[test]
    fn sqrt_test() {
        is_power_of_two(0) == false;
        let r = Request { inner: Option::Some(Inner { color: Size::Large }) } ;
        let result = ShirtsOracle::shirt(r);

        assert!(result.color == Size::Large);
    }
}
