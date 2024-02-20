mod oracle;

use oracle::{BinaryOracle, Request, Response};

// Check `x` is 0 or 1
fn is_boolean(x: u32) -> bool {
    (1-x)*(x) == 0
}

fn binary_decomposition_correct(input_dec: u32, num_bits: u32, input_bin: Array<u32>) -> bool {
    // Verify the lenth of the returned array matches the expected length `num_bits`
    if input_bin.len() != num_bits {
        return false;
    }

    let nb_zero = *input_bin.at(0);
    let nb_one = *input_bin.at(1);
    let nb_two = *input_bin.at(2);

    // Verify each element in the returned array is boolean (0 or 1)
    if !is_boolean(nb_zero) {
        return false;
    }

    if !is_boolean(nb_one) {
        return false;
    }

    if !is_boolean(nb_two) {
        return false;
    }

    // Convert from binary to decimal
    let recover_bin = nb_zero * 1 + nb_one * 2 + nb_two * 4;

    // Verify the recovered binary matches the input to the oracle
    if recover_bin != input_dec {
        return false;
    } else {
        return true;
    }
}

fn main() -> bool {
    let n_input = 5;
    let bin_len = 3;
    let max_int = (2*2*2)-1; // (2^bin_len)-1

    // Check input fits in `bin_len` bits
    if n_input > max_int {
        return false;
    }

    let req = Request { n: n_input, len: bin_len } ;
    let result = BinaryOracle::to_binary(req);
    let n_binary: Array<u32> = result.nb;

    if !binary_decomposition_correct(n_input, bin_len, n_binary) {
        return false;
    }

    // Now it's safe to use `n_binary`

    return true;
}

#[cfg(test)]
mod tests {
    //use super::{Request, Response, Size, ShirtsOracle, Inner, is_power_of_two};

    #[test]
    fn sqrt_test() {
        
    }
}