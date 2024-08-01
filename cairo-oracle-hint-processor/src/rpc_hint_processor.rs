use super::Error;
use crate::hint_processor_utils::{cell_ref_to_relocatable, extract_buffer, get_ptr};
use crate::insert_value_to_cellref;
use cairo_lang_casm::{
    hints::{Hint, StarknetHint},
    operand::{CellRef, ResOperand},
};
use cairo_lang_utils::bigint::BigIntAsHex;
use cairo_proto_serde::configuration::{Configuration, PollingConfig};
use cairo_proto_serde::{deserialize_cairo_serde, serialize_cairo_serde};
use cairo_vm::hint_processor::cairo_1_hint_processor::hint_processor::Cairo1HintProcessor;
use cairo_vm::hint_processor::hint_processor_definition::HintProcessorLogic;
use cairo_vm::hint_processor::hint_processor_definition::HintReference;
use cairo_vm::vm::errors::vm_errors::VirtualMachineError;
use cairo_vm::vm::runners::cairo_runner::ResourceTracker;
use cairo_vm::Felt252;
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
use core::any::Any;
use reqwest::Url;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// HintProcessor for Cairo 1 compiler hints.
pub struct Rpc1HintProcessor<'a> {
    inner_processor: Cairo1HintProcessor,
    configuration: &'a Configuration,
}

impl<'a> Rpc1HintProcessor<'a> {
    pub fn new(
        inner_processor: Cairo1HintProcessor,
        configuration: &'a Configuration,
    ) -> Result<Self, Error> {
        Ok(Self {
            inner_processor,
            configuration,
        })
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
            HintError::CustomHint(Box::from("Failed to parse selector".to_string()))
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

        let server_config = self
            .configuration
            .servers_config
            .get(selector)
            .ok_or_else(|| {
                HintError::CustomHint(Box::from(format!(
                    "No server URL configured for selector: {selector}"
                )))
            })?;

        let mut server_url = Url::parse(&server_config.server_url).map_err(|e| {
            HintError::CustomHint(Box::from(format!(
                "Invalid URL for selector {selector}: {e}"
            )))
        })?;
        server_url
            .path_segments_mut()
            .expect("cannot be a base URL")
            .push(selector);

        let data = deserialize_cairo_serde(
            self.configuration,
            &configuration.input,
            &mut inputs.as_ref(),
        );
        println!("let the oracle decide... Inputs: {data:?}");

        let use_polling = server_config.polling.unwrap_or(false);

        if use_polling {
            let default_polling_config = PollingConfig {
                max_attempts: 30,
                polling_interval: 2,
                request_timeout: 10,
                overall_timeout: 60,
            };

            let polling_config = server_config
                .polling_config
                .as_ref()
                .unwrap_or(&default_polling_config);

            let client = reqwest::blocking::ClientBuilder::new()
                .timeout(Duration::from_secs(polling_config.request_timeout))
                .build()
                .map_err(|e| {
                    HintError::CustomHint(Box::from(format!("Failed to create HTTP client: {}", e)))
                })?;

            let max_attempts = polling_config.max_attempts;
            let polling_interval = Duration::from_secs(polling_config.polling_interval);
            let start_time = Instant::now();
            let overall_timeout = Duration::from_secs(polling_config.overall_timeout);

            // Initial request to start the job
            let response = client
                .post(server_url.clone())
                .json(&data)
                .header("x-admin-api-key", "qwerty")
                .send()
                .map_err(|e| {
                    HintError::CustomHint(Box::from(format!(
                        "Failed to send request to oracle server {}: {}",
                        server_url, e
                    )))
                })?;

            let response_body = response.text().map_err(|e| {
                HintError::CustomHint(Box::from(format!("Failed to get response body: {}", e)))
            })?;

            println!("Initial response body: {}", response_body);

            let response_json: serde_json::Value =
                serde_json::from_str(&response_body).map_err(|e| {
                    HintError::CustomHint(Box::from(format!(
                        "Failed to parse response JSON: {}",
                        e
                    )))
                })?;

            let job_id = response_json
                .get("jobId")
                .and_then(|v| v.as_str())
                .ok_or_else(|| HintError::CustomHint(Box::from("Failed to get jobId")))?
                .to_string();

            println!("Received job_id: {}", job_id);

            let mut attempt = 0;
            loop {
                if attempt >= max_attempts || start_time.elapsed() > overall_timeout {
                    return Err(HintError::CustomHint(Box::from(
                        "Polling timed out".to_string(),
                    )));
                }

                let status_url = server_url
                    .join(&format!("status/{}", job_id))
                    .map_err(|e| {
                        HintError::CustomHint(Box::from(format!(
                            "Failed to construct status URL: {}",
                            e
                        )))
                    })?;

                println!("Checking status at URL: {}", status_url);

                let status_response = client.get(status_url.clone()).send().map_err(|e| {
                    HintError::CustomHint(Box::from(format!(
                        "Failed to send status request: {}",
                        e
                    )))
                })?;

                let status_body = status_response.text().map_err(|e| {
                    HintError::CustomHint(Box::from(format!(
                        "Failed to get status response body: {}",
                        e
                    )))
                })?;

                println!("Status response body: {}", status_body);

                let status_json: serde_json::Value = match serde_json::from_str(&status_body) {
                    Ok(json) => json,
                    Err(e) => {
                        println!(
                            "Failed to parse status JSON: {}. Raw response: {}",
                            e, status_body
                        );
                        return Err(HintError::CustomHint(Box::from(format!(
                            "Failed to parse status JSON: {}. Raw response: {}",
                            e, status_body
                        ))));
                    }
                };

                if status_json.get("status").and_then(|s| s.as_str()) == Some("completed") {
                    if let Some(output) = status_json.get("result") {
                        let data = serialize_cairo_serde(
                            self.configuration,
                            &configuration.output,
                            output,
                        );
                        println!("Output: {output}");
                        res_segment.write_data(data.iter()).map_err(|e| {
                            HintError::CustomHint(Box::from(format!(
                                "Failed to write data to result segment: {}",
                                e
                            )))
                        })?;

                        let res_segment_end = res_segment.ptr;
                        insert_value_to_cellref!(vm, output_start, res_segment_start).map_err(
                            |e| {
                                HintError::CustomHint(Box::from(format!(
                                    "Failed to insert output start value: {}",
                                    e
                                )))
                            },
                        )?;
                        insert_value_to_cellref!(vm, output_end, res_segment_end).map_err(|e| {
                            HintError::CustomHint(Box::from(format!(
                                "Failed to insert output end value: {}",
                                e
                            )))
                        })?;

                        return Ok(());
                    }
                } else {
                    println!("Job not completed. Current status: {:?}", status_json);
                }

                std::thread::sleep(polling_interval);
                attempt += 1;
            }
        } else {
            let client = reqwest::blocking::Client::new();
            let response = client
                .post(server_url.clone())
                .json(&data)
                .header("x-admin-api-key", "qwerty")
                .send()
                .map_err(|e| {
                    HintError::CustomHint(Box::from(format!(
                        "Failed to send request to oracle server {}: {}",
                        server_url, e
                    )))
                })?;

            let response_body = response.text().map_err(|e| {
                HintError::CustomHint(Box::from(format!("Failed to get response body: {}", e)))
            })?;

            let response_json: serde_json::Value =
                serde_json::from_str(&response_body).map_err(|e| {
                    HintError::CustomHint(Box::from(format!(
                        "Failed to parse response JSON: {}",
                        e
                    )))
                })?;
            print!("Response: {response_json}");
            let output = if response_json.is_object() {
                response_json
            } else {
                return Err(HintError::CustomHint(Box::from(format!(
                    "Unexpected response format. Expected an object, got: {:?}",
                    response_json
                ))));
            };            

            let data = serialize_cairo_serde(self.configuration, &configuration.output, &output);
            res_segment.write_data(data.iter()).map_err(|e| {
                HintError::CustomHint(Box::from(format!(
                    "Failed to write data to result segment: {}",
                    e
                )))
            })?;

            let res_segment_end = res_segment.ptr;
            insert_value_to_cellref!(vm, output_start, res_segment_start).map_err(|e| {
                HintError::CustomHint(Box::from(format!(
                    "Failed to insert output start value: {}",
                    e
                )))
            })?;
            insert_value_to_cellref!(vm, output_end, res_segment_end).map_err(|e| {
                HintError::CustomHint(Box::from(format!(
                    "Failed to insert output end value: {}",
                    e
                )))
            })?;
        }

        Ok(())
    }
}

impl<'a> HintProcessorLogic for Rpc1HintProcessor<'a> {
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
        _constants: &std::collections::HashMap<String, Felt252>,
    ) -> Result<(), cairo_vm::vm::errors::hint_errors::HintError> {
        let hints: &Vec<Hint> = hint_data.downcast_ref().ok_or(HintError::WrongHintData)?;

        for hint in hints {
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
                    self.inner_processor.execute(vm, exec_scopes, hint)?;
                }
            }
        }
        Ok(())
    }
}

impl<'a> ResourceTracker for Rpc1HintProcessor<'a> {
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
fn extract_relocatable(
    vm: &VirtualMachine,
    buffer: &ResOperand,
) -> Result<Relocatable, VirtualMachineError> {
    let (base, offset) = extract_buffer(buffer).unwrap();
    get_ptr(vm, base, &offset)
}

/// Loads a range of values from the VM memory.
fn vm_get_range(
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
trait VMWrapper {
    fn vm(&mut self) -> &mut VirtualMachine;
}
impl VMWrapper for VirtualMachine {
    fn vm(&mut self) -> &mut VirtualMachine {
        self
    }
}

/// A helper struct to continuously write and read from a buffer in the VM memory.
struct MemBuffer<'a> {
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
