mod hint_processor_utils;

use core::any::Any;
use std::{collections::HashMap, io::Read};

use cairo_lang_casm::{
    hints::{Hint, StarknetHint},
    operand::{CellRef, ResOperand},
};
use cairo_lang_runner::{CairoHintProcessor, StarknetState};
use cairo_lang_utils::bigint::BigIntAsHex;
use cairo_proto_serde::configuration::Configuration;
use cairo_proto_serde::{deserialize_cairo_serde, serialize_cairo_serde};
use cairo_vm::hint_processor::hint_processor_definition::HintReference;
use cairo_vm::vm::runners::cairo_runner::ResourceTracker;
use cairo_vm::{
    felt::Felt252, hint_processor::hint_processor_definition::HintProcessorLogic,
    vm::errors::vm_errors::VirtualMachineError,
};
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
use hint_processor_utils::{cell_ref_to_relocatable, extract_buffer, get_ptr};
use num_traits::ToPrimitive;
use serde_json::{json, Map, Value};

#[derive(Debug, PartialEq)]
enum PathElement {
    Struct,
    Array,
    Key(String),
}

#[derive(Debug)]
enum OracleState {
    Sending(Value),
    Receiving(Value),
}

/// HintProcessor for Cairo 1 compiler hints.
pub struct RpcHintProcessor<'a> {
    path: Vec<PathElement>,
    state: OracleState,
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
            state: OracleState::Sending(Value::Null),
            path: Default::default(),
            inner_processor,
            server: server.clone(),
            configuration,
        }
    }

    // fn to_string(&self, felt: &Felt252) -> String {
    //     let by = felt.to_bigint().to_bytes_be().1;
    //     String::from_utf8(by).unwrap()
    // }

    // fn set_key(val_ref: &mut Map<String, Value>, remaining_path: &[PathElement], value: Value) {
    //     println!("set_key {val_ref:?} {remaining_path:?} {value:?}");

    //     let Some(PathElement::Key(key)) = remaining_path.first() else {
    //         return;
    //     };

    //     let remaining_path = &remaining_path[1..];
    //     if remaining_path.len() == 0 {
    //         val_ref.insert(key.clone(), value);
    //     } else {
    //         match val_ref.get_mut(key) {
    //             Some(val_ref) => {
    //                 Self::set(val_ref, remaining_path, value);
    //             }
    //             None => {
    //                 val_ref.insert(key.clone(), Value::Null);
    //                 let val_ref = val_ref.get_mut(key).unwrap();
    //                 Self::set(val_ref, remaining_path, value);
    //             }
    //         }
    //     }
    // }

    // fn get_key(val_ref: &Map<String, Value>, remaining_path: &[PathElement]) -> Value {
    //     println!("get_key {val_ref:?} {remaining_path:?}");

    //     let Some(PathElement::Key(key)) = remaining_path.first() else {
    //         return Value::Null;
    //     };

    //     let remaining_path = &remaining_path[1..];
    //     if remaining_path.len() == 0 {
    //         val_ref.get(key).unwrap().clone()
    //     } else {
    //         Self::get(val_ref.get(key).unwrap(), remaining_path)
    //     }
    // }

    // fn set(val_ref: &mut Value, remaining_path: &[PathElement], value: Value) {
    //     println!("set {val_ref:?} {remaining_path:?} {value:?}");

    //     let Some(step) = remaining_path.first() else {
    //         return;
    //     };

    //     match step {
    //         PathElement::Struct => {
    //             if matches!(val_ref, Value::Null) {
    //                 *val_ref = Value::Object(Default::default());
    //             }
    //             if let Value::Object(inner) = val_ref {
    //                 Self::set_key(inner, &remaining_path[1..], value);
    //             } else {
    //                 panic!("incompatible type already set");
    //             }
    //         }
    //         PathElement::Array => todo!(),
    //         PathElement::Key(_) => todo!(),
    //     }
    // }

    // fn get(val_ref: &Value, remaining_path: &[PathElement]) -> Value {
    //     println!("get {val_ref:?} {remaining_path:?}");

    //     let Some(step) = remaining_path.first() else {
    //         return Value::Null;
    //     };

    //     match step {
    //         PathElement::Struct => {
    //             if let Value::Object(inner) = val_ref {
    //                 Self::get_key(inner, &remaining_path[1..])
    //             } else {
    //                 panic!("incompatible type already set");
    //             }
    //         }
    //         PathElement::Array => todo!(),
    //         PathElement::Key(_) => todo!(),
    //     }
    // }

    // fn set_value(&mut self, value: Value) {
    //     match &mut self.state {
    //         OracleState::Sending(state) => {
    //             Self::set(state, &self.path, value);
    //         }
    //         _ => {
    //             panic!("cannot set value when not sending");
    //         }
    //     }
    // }

    // fn get_value(&self) -> Value {
    //     match &self.state {
    //         OracleState::Receiving(state) => Self::get(state, &self.path),
    //         _ => {
    //             panic!("cannot get value when not receiving");
    //         }
    //     }
    // }

    /// Executes a cheatcode.
    fn execute_cheatcode(
        &mut self,
        selector: &BigIntAsHex,
        [input_start, input_end]: [&ResOperand; 2],
        [output_start, output_end]: [&CellRef; 2],
        vm: &mut VirtualMachine,
        exec_scopes: &mut ExecutionScopes,
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

        let Some(configuration) = self.configuration.services.iter().find_map(|(_, methods)| {
            methods.methods.get(selector)
        }) else {
            return Err(HintError::CustomHint(Box::from(format!(
                "Unknown cheatcode selector: {selector}"
            ))));
        };

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

        // let contract_logs = self.starknet_state.logs.get_mut(&as_single_input(inputs)?);
        // if let Some((keys, data)) =
        //     contract_logs.and_then(|contract_logs| contract_logs.events.pop_front())
        // {
        //     res_segment.write(keys.len())?;
        //     res_segment.write_data(keys.iter())?;
        //     res_segment.write(data.len())?;
        //     res_segment.write_data(data.iter())?;
        // }

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
        // println!("Hint {:#?}", hint);
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

// TODO: copied from cairo-lang-runner

// pub fn cell_ref_to_relocatable(cell_ref: &CellRef, vm: &VirtualMachine) -> Relocatable {
//     let base = match cell_ref.register {
//         Register::AP => vm.get_ap(),
//         Register::FP => vm.get_fp(),
//     };
//     (base + (cell_ref.offset as i32)).unwrap()
// }

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
