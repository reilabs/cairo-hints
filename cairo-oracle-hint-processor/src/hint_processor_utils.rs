use cairo_lang_casm::operand::{CellRef, DerefOrImmediate, Register, ResOperand};
use cairo_vm::types::{errors::math_errors::MathError, relocatable::Relocatable};
use cairo_vm::utils::bigint_to_felt;
use cairo_vm::Felt252;

use cairo_vm::vm::errors::{hint_errors::HintError, vm_errors::VirtualMachineError};
use cairo_vm::vm::vm_core::VirtualMachine;

/// Inserts a value into the vm memory cell represented by the cellref.
#[macro_export]
macro_rules! insert_value_to_cellref {
    ($vm:ident, $cell_ref:ident, $value:expr) => {
        // TODO: unwrap?
        $vm.insert_value(cell_ref_to_relocatable($cell_ref, $vm).unwrap(), $value)
    };
}

/// Extracts a parameter assumed to be a buffer.
pub fn extract_buffer(buffer: &ResOperand) -> Result<(&CellRef, Felt252), HintError> {
    let (cell, base_offset) = match buffer {
        ResOperand::Deref(cell) => (cell, 0.into()),
        ResOperand::BinOp(bin_op) => {
            if let DerefOrImmediate::Immediate(val) = &bin_op.b {
                (&bin_op.a, bigint_to_felt(&val.value)?)
            } else {
                return Err(HintError::CustomHint(
                    "Failed to extract buffer, expected ResOperand of BinOp type to have Inmediate b value".to_owned().into_boxed_str()
                ));
            }
        }
        _ => {
            return Err(HintError::CustomHint(
                "Illegal argument for a buffer."
                    .to_string()
                    .into_boxed_str(),
            ))
        }
    };
    Ok((cell, base_offset))
}

pub(crate) fn cell_ref_to_relocatable(
    cell_ref: &CellRef,
    vm: &VirtualMachine,
) -> Result<Relocatable, MathError> {
    let base = match cell_ref.register {
        Register::AP => vm.get_ap(),
        Register::FP => vm.get_fp(),
    };
    base + (cell_ref.offset as i32)
}

pub(crate) fn get_ptr(
    vm: &VirtualMachine,
    cell: &CellRef,
    offset: &Felt252,
) -> Result<Relocatable, VirtualMachineError> {
    Ok((vm.get_relocatable(cell_ref_to_relocatable(cell, vm)?)? + offset)?)
}

#[cfg(feature = "std")]
pub(crate) fn as_relocatable(
    vm: &mut VirtualMachine,
    value: &ResOperand,
) -> Result<Relocatable, HintError> {
    let (base, offset) = extract_buffer(value)?;
    get_ptr(vm, base, &offset).map_err(HintError::from)
}

#[cfg(test)]
pub(crate) fn as_cairo_short_string(value: &Felt252) -> Option<String> {
    let mut as_string = String::default();
    let mut is_end = false;
    for byte in value
        .to_bytes_be()
        .into_iter()
        .skip_while(num_traits::Zero::is_zero)
    {
        if byte == 0 {
            is_end = true;
        } else if is_end || !byte.is_ascii() {
            return None;
        } else {
            as_string.push(byte as char);
        }
    }
    Some(as_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn simple_as_cairo_short_string() {
        // Values extracted from cairo book example
        let s = "Hello, Scarb!";
        let x = Felt252::from(5735816763073854913753904210465_u128);
        assert!(s.is_ascii());
        let cairo_string = as_cairo_short_string(&x).expect("call to as_cairo_short_string failed");
        assert_eq!(cairo_string, s);
    }
}
