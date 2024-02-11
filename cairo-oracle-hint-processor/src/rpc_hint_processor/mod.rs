use core::any::Any;
use std::collections::HashMap;

use crate::hint_processor_utils::{cell_ref_to_relocatable, extract_buffer, get_ptr};
use crate::insert_value_to_cellref;
use cairo_lang_casm::{
    hints::{Hint, StarknetHint},
    operand::{CellRef, ResOperand},
};
use cairo_lang_runner::{CairoHintProcessor, StarknetState};
use cairo_lang_utils::bigint::BigIntAsHex;
use cairo_proto_serde::configuration::Configuration;
use cairo_proto_serde::{deserialize_cairo_serde, serialize_cairo_serde};
use cairo_vm::felt::Felt252;
use cairo_vm::hint_processor::hint_processor_definition::HintProcessorLogic;
use cairo_vm::hint_processor::hint_processor_definition::HintReference;
use cairo_vm::vm::errors::vm_errors::VirtualMachineError;
use cairo_vm::vm::runners::cairo_runner::ResourceTracker;
use cairo_vm::{
    types::{
        exec_scope::ExecutionScopes,
        relocatable::{MaybeRelocatable, Relocatable},
    },
    vm::{
        errors::{hint_errors::HintError, memory_errors::MemoryError},
        vm_core::VirtualMachine,
    },
};
use serde_json::Value;

/// HintProcessor for Cairo 1 compiler hints.
pub struct RpcHintProcessor<'a> {
    inner_processor: CairoHintProcessor<'a>,
    server: Option<String>,
    configuration: &'a Configuration,
}

impl<'a> RpcHintProcessor<'a> {
    pub fn new(
        inner_processor: CairoHintProcessor<'a>,
        server: &Option<String>,
        configuration: &'a Configuration,
    ) -> Self {
        Self {
            inner_processor,
            server: server.clone(),
            configuration,
        }
    }

    /// Executes a cheatcode.
    fn execute_cheatcode(
        &mut self,
        selector: &BigIntAsHex,
        [input_start, input_end]: [&ResOperand; 2],
        [output_start, output_end]: [&CellRef; 2],
        vm: &mut VirtualMachine,
        _exec_scopes: &mut ExecutionScopes,
    ) -> Result<(), HintError> {
        // Parse the selector.
        let selector = &selector.value.to_bytes_be().1;
        let selector = std::str::from_utf8(selector).map_err(|_| {
            HintError::CustomHint(Box::from("failed to parse selector".to_string()))
        })?;

        // Extract the inputs.
        let input_start = extract_relocatable(vm, input_start)?;
        let input_end = extract_relocatable(vm, input_end)?;
        let inputs = vm_get_range(vm, input_start, input_end)?;

        let mut res_segment = MemBuffer::new_segment(vm);
        let res_segment_start = res_segment.ptr;

        let Some(configuration) = self
            .configuration
            .services
            .iter()
            .find_map(|(_, methods)| methods.methods.get(selector))
        else {
            return Err(HintError::CustomHint(Box::from(format!(
                "Unknown cheatcode selector: {selector}"
            ))));
        };

        // println!("Configuration {:#?}", self.configuration);

        let data = deserialize_cairo_serde(
            self.configuration,
            &configuration.input,
            &mut inputs.as_ref(),
        );
        println!("let the oracle decide... Inputs: {data:?}");

        let client = reqwest::blocking::Client::new();
        let resp = client
            .post(self.server.as_ref().unwrap())
            .json(&data)
            .send()
            .unwrap()
            .json::<Value>()
            .unwrap();

        let output = &resp["result"];
        let data = serialize_cairo_serde(self.configuration, &configuration.output, output);
        println!("Output: {output}");
        res_segment.write_data(data.iter())?;

        let res_segment_end = res_segment.ptr;
        insert_value_to_cellref!(vm, output_start, res_segment_start)?;
        insert_value_to_cellref!(vm, output_end, res_segment_end)?;
        Ok(())
    }

    pub fn starknet_state(self) -> StarknetState {
        self.inner_processor.starknet_state
    }
}

impl<'a> HintProcessorLogic for RpcHintProcessor<'a> {
    // Ignores all data except for the code that should contain
    fn compile_hint(
        &self,
        //Block of hint code as String
        hint_code: &str,
        //Ap Tracking Data corresponding to the Hint
        ap_tracking_data: &cairo_vm::serde::deserialize_program::ApTracking,
        //Map from variable name to reference id number
        //(may contain other variables aside from those used by the hint)
        reference_ids: &HashMap<String, usize>,
        //List of all references (key corresponds to element of the previous dictionary)
        references: &[HintReference],
    ) -> Result<Box<dyn Any>, VirtualMachineError> {
        self.inner_processor
            .compile_hint(hint_code, ap_tracking_data, reference_ids, references)
    }

    fn execute_hint(
        &mut self,
        vm: &mut cairo_vm::vm::vm_core::VirtualMachine,
        exec_scopes: &mut cairo_vm::types::exec_scope::ExecutionScopes,
        //Data structure that can be downcasted to the structure generated by compile_hint
        hint_data: &Box<dyn core::any::Any>,
        //Constant values extracted from the program specification.
        constants: &std::collections::HashMap<String, Felt252>,
    ) -> Result<(), cairo_vm::vm::errors::hint_errors::HintError> {
        let hint = hint_data.downcast_ref::<Hint>().unwrap();
        match hint {
            Hint::Starknet(StarknetHint::Cheatcode {
                selector,
                input_start,
                input_end,
                output_start,
                output_end,
            }) => {
                self.execute_cheatcode(
                    selector,
                    [input_start, input_end],
                    [output_start, output_end],
                    vm,
                    exec_scopes,
                )?;
            }
            _ => {
                self.inner_processor
                    .execute_hint(vm, exec_scopes, hint_data, constants)?;
            }
        }
        Ok(())
    }
}

impl<'a> ResourceTracker for RpcHintProcessor<'a> {
    fn consumed(&self) -> bool {
        self.inner_processor.consumed()
    }

    fn consume_step(&mut self) {
        self.inner_processor.consume_step()
    }

    fn get_n_steps(&self) -> Option<usize> {
        self.inner_processor.get_n_steps()
    }

    fn run_resources(&self) -> &cairo_vm::vm::runners::cairo_runner::RunResources {
        self.inner_processor.run_resources()
    }
}

/// Extracts a parameter assumed to be a buffer, and converts it into a relocatable.
pub fn extract_relocatable(
    vm: &VirtualMachine,
    buffer: &ResOperand,
) -> Result<Relocatable, VirtualMachineError> {
    let (base, offset) = extract_buffer(buffer).unwrap();
    get_ptr(vm, base, &offset)
}

/// Loads a range of values from the VM memory.
pub fn vm_get_range(
    vm: &mut VirtualMachine,
    mut calldata_start_ptr: Relocatable,
    calldata_end_ptr: Relocatable,
) -> Result<Vec<Felt252>, HintError> {
    let mut values = vec![];
    while calldata_start_ptr != calldata_end_ptr {
        let val = vm.get_integer(calldata_start_ptr)?.into_owned();
        values.push(val);
        calldata_start_ptr.offset += 1;
    }
    Ok(values)
}

/// Wrapper trait for a VM owner.
pub trait VMWrapper {
    fn vm(&mut self) -> &mut VirtualMachine;
}
impl VMWrapper for VirtualMachine {
    fn vm(&mut self) -> &mut VirtualMachine {
        self
    }
}

/// A helper struct to continuously write and read from a buffer in the VM memory.
pub struct MemBuffer<'a> {
    /// The VM to write to.
    /// This is a trait so that we would borrow the actual VM only once.
    vm: &'a mut dyn VMWrapper,
    /// The current location of the buffer.
    pub ptr: Relocatable,
}
impl<'a> MemBuffer<'a> {
    /// Creates a new buffer.
    pub fn new(vm: &'a mut dyn VMWrapper, ptr: Relocatable) -> Self {
        Self { vm, ptr }
    }

    /// Creates a new segment and returns a buffer wrapping it.
    pub fn new_segment(vm: &'a mut dyn VMWrapper) -> Self {
        let ptr = vm.vm().add_memory_segment();
        Self::new(vm, ptr)
    }

    /// Returns the current position of the buffer and advances it by one.
    fn next(&mut self) -> Relocatable {
        let ptr = self.ptr;
        self.ptr += 1;
        ptr
    }

    /// Writes a value to the current position of the buffer and advances it by one.
    pub fn write<T: Into<MaybeRelocatable>>(&mut self, value: T) -> Result<(), MemoryError> {
        let ptr = self.next();
        self.vm.vm().insert_value(ptr, value)
    }

    /// Writes an iterator of values starting from the current position of the buffer and advances
    /// it to after the end of the written value.
    pub fn write_data<T: Into<MaybeRelocatable>, Data: Iterator<Item = T>>(
        &mut self,
        data: Data,
    ) -> Result<(), MemoryError> {
        for value in data {
            self.write(value)?;
        }
        Ok(())
    }
}
