use cairo_lang_casm::{casm, casm_extend, hints::Hint, instructions::Instruction};
use cairo_lang_sierra::{
    extensions::{
        bitwise::BitwiseType,
        core::{CoreLibfunc, CoreType},
        ec::EcOpType,
        gas::{CostTokenType, GasBuiltinType},
        pedersen::PedersenType,
        poseidon::PoseidonType,
        range_check::RangeCheckType,
        segment_arena::SegmentArenaType,
        starknet::syscalls::SystemType,
        ConcreteType, NamedType,
    },
    ids::ConcreteTypeId,
    program::{Function, Program as SierraProgram},
    program_registry::ProgramRegistry,
};
use cairo_lang_sierra_ap_change::calc_ap_changes;
use cairo_lang_sierra_gas::gas_info::GasInfo;
use cairo_lang_sierra_to_casm::{
    compiler::CairoProgram,
    metadata::{calc_metadata, Metadata, MetadataComputationConfig, MetadataError},
};
use cairo_lang_sierra_type_size::get_type_size_map;
use cairo_lang_utils::unordered_hash_map::UnorderedHashMap;
use cairo_proto_serde::configuration::Configuration;
use cairo_vm::{
    hint_processor::cairo_1_hint_processor::hint_processor::Cairo1HintProcessor,
    serde::deserialize_program::{
        ApTracking, BuiltinName, FlowTrackingData, HintParams, ReferenceManager,
    },
    types::{program::Program, relocatable::MaybeRelocatable},
    vm::{
        errors::{runner_errors::RunnerError, vm_errors::VirtualMachineError},
        runners::{
            builtin_runner::{
                BITWISE_BUILTIN_NAME, EC_OP_BUILTIN_NAME, HASH_BUILTIN_NAME, OUTPUT_BUILTIN_NAME,
                POSEIDON_BUILTIN_NAME, RANGE_CHECK_BUILTIN_NAME, SIGNATURE_BUILTIN_NAME,
            },
            cairo_runner::{CairoRunner, RunResources, RunnerMode},
        },
        vm_core::VirtualMachine,
    },
    Felt252,
};
use itertools::chain;
use std::collections::HashMap;

use crate::{rpc_hint_processor::Rpc1HintProcessor, Error, FuncArg};

#[derive(Debug)]
pub struct Cairo1RunConfig<'a> {
    pub args: &'a [FuncArg],
    pub trace_enabled: bool,
    pub relocate_mem: bool,
    pub layout: &'a str,
    pub proof_mode: bool,
    // Should be true if either air_public_input or cairo_pie_output are needed
    // Sets builtins stop_ptr by calling `final_stack` on each builtin
    pub finalize_builtins: bool,
}

impl Default for Cairo1RunConfig<'_> {
    fn default() -> Self {
        Self {
            args: Default::default(),
            trace_enabled: false,
            relocate_mem: false,
            layout: "plain",
            proof_mode: false,
            finalize_builtins: false,
        }
    }
}

// Runs a Cairo 1 program
// Returns the runner & VM after execution + the return values
pub fn cairo_run_program(
    sierra_program: &SierraProgram,
    cairo_run_config: Cairo1RunConfig,
    service_config: &Configuration,
    oracle_server: &Option<String>,
    entry_func_name: &str,
) -> Result<(CairoRunner, VirtualMachine, Vec<MaybeRelocatable>), Error> {
    let metadata = create_metadata(sierra_program, Some(Default::default()))?;
    let sierra_program_registry = ProgramRegistry::<CoreType, CoreLibfunc>::new(sierra_program)?;
    let type_sizes =
        get_type_size_map(sierra_program, &sierra_program_registry).unwrap_or_default();
    let casm_program =
        cairo_lang_sierra_to_casm::compiler::compile(sierra_program, &metadata, true)?;

    let main_func = find_function(sierra_program, entry_func_name)?;

    let initial_gas = 9999999999999_usize;

    // Modified entry code to be compatible with custom cairo1 Proof Mode.
    // This adds code that's needed for dictionaries, adjusts ap for builtin pointers, adds initial gas for the gas builtin if needed, and sets up other necessary code for cairo1
    let (entry_code, builtins) = create_entry_code(
        &sierra_program_registry,
        &casm_program,
        &type_sizes,
        main_func,
        initial_gas,
        cairo_run_config.proof_mode,
        cairo_run_config.args,
    )?;

    // Fetch return type data
    let return_type_id = main_func
        .signature
        .ret_types
        .last()
        .ok_or(Error::NoRetTypesInSignature)?;
    let return_type_size = type_sizes
        .get(return_type_id)
        .cloned()
        .ok_or_else(|| Error::NoTypeSizeForId(return_type_id.clone()))?;

    // This footer is used by lib funcs
    let libfunc_footer = create_code_footer();

    // Header used to initiate the infinite loop after executing the program
    // Also appends return values to output segment
    let proof_mode_header = if cairo_run_config.proof_mode {
        create_proof_mode_header(builtins.len() as i16, return_type_size)
    } else {
        casm! {}.instructions
    };

    // This is the program we are actually running/proving
    // With (embedded proof mode), cairo1 header and the libfunc footer
    let instructions = chain!(
        proof_mode_header.iter(),
        entry_code.iter(),
        casm_program.instructions.iter(),
        libfunc_footer.iter(),
    );

    let (processor_hints, program_hints) = build_hints_vec(instructions.clone());

    let hint_processor = Cairo1HintProcessor::new(&processor_hints, RunResources::default());
    let mut hint_processor = Rpc1HintProcessor::new(hint_processor, oracle_server, service_config);

    let data: Vec<MaybeRelocatable> = instructions
        .flat_map(|inst| inst.assemble().encode())
        .map(|x| Felt252::from(&x))
        .map(MaybeRelocatable::from)
        .collect();

    let data_len = data.len();

    let program = if cairo_run_config.proof_mode {
        Program::new_for_proof(
            builtins,
            data,
            0,
            // Proof mode is on top
            // jmp rel 0 is on PC == 2
            2,
            program_hints,
            ReferenceManager {
                references: Vec::new(),
            },
            HashMap::new(),
            vec![],
            None,
        )?
    } else {
        Program::new(
            builtins,
            data,
            Some(0),
            program_hints,
            ReferenceManager {
                references: Vec::new(),
            },
            HashMap::new(),
            vec![],
            None,
        )?
    };

    let runner_mode = if cairo_run_config.proof_mode {
        RunnerMode::ProofModeCairo1
    } else {
        RunnerMode::ExecutionMode
    };

    let mut runner = CairoRunner::new_v2(&program, cairo_run_config.layout, runner_mode)?;
    let mut vm = VirtualMachine::new(cairo_run_config.trace_enabled);
    let end = runner.initialize(&mut vm, cairo_run_config.proof_mode)?;

    additional_initialization(&mut vm, data_len)?;

    // Run it until the end / infinite loop in proof_mode
    runner.run_until_pc(end, &mut vm, &mut hint_processor)?;
    if cairo_run_config.proof_mode {
        // As we will be inserting the return values into the output segment after running the main program (right before the infinite loop) the computed size for the output builtin will be 0
        // We need to manually set the segment size for the output builtin's segment so memory hole counting doesn't fail due to having a higher accessed address count than the segment's size
        vm.segments
            .segment_sizes
            .insert(2, return_type_size as usize);
    }
    runner.end_run(false, false, &mut vm, &mut hint_processor)?;

    // Fetch return values
    let return_values = fetch_return_values(return_type_size, return_type_id, &vm)?;

    // Set stop pointers for builtins so we can obtain the air public input
    if cairo_run_config.finalize_builtins {
        finalize_builtins(
            cairo_run_config.proof_mode,
            &main_func.signature.ret_types,
            &type_sizes,
            &mut vm,
        )?;

        // Build execution public memory
        if cairo_run_config.proof_mode {
            // As the output builtin is not used by the program we need to compute it's stop ptr manually
            vm.set_output_stop_ptr_offset(return_type_size as usize);

            runner.finalize_segments(&mut vm)?;
        }
    }

    runner.relocate(&mut vm, true)?;

    Ok((runner, vm, return_values))
}

fn additional_initialization(vm: &mut VirtualMachine, data_len: usize) -> Result<(), Error> {
    // Create the builtin cost segment
    let builtin_cost_segment = vm.add_memory_segment();
    for token_type in CostTokenType::iter_precost() {
        vm.insert_value(
            (builtin_cost_segment + (token_type.offset_in_builtin_costs() as usize))
                .map_err(VirtualMachineError::Math)?,
            Felt252::default(),
        )?
    }
    // Put a pointer to the builtin cost segment at the end of the program (after the
    // additional `ret` statement).
    vm.insert_value(
        (vm.get_pc() + data_len).map_err(VirtualMachineError::Math)?,
        builtin_cost_segment,
    )?;

    Ok(())
}

#[allow(clippy::type_complexity)]
fn build_hints_vec<'b>(
    instructions: impl Iterator<Item = &'b Instruction>,
) -> (Vec<(usize, Vec<Hint>)>, HashMap<usize, Vec<HintParams>>) {
    let mut hints: Vec<(usize, Vec<Hint>)> = Vec::new();
    let mut program_hints: HashMap<usize, Vec<HintParams>> = HashMap::new();

    let mut hint_offset = 0;

    for instruction in instructions {
        if !instruction.hints.is_empty() {
            hints.push((hint_offset, instruction.hints.clone()));
            program_hints.insert(
                hint_offset,
                vec![HintParams {
                    code: hint_offset.to_string(),
                    accessible_scopes: Vec::new(),
                    flow_tracking_data: FlowTrackingData {
                        ap_tracking: ApTracking::default(),
                        reference_ids: HashMap::new(),
                    },
                }],
            );
        }
        hint_offset += instruction.body.op_size();
    }
    (hints, program_hints)
}

/// Finds first function ending with `name_suffix`.
fn find_function<'a>(
    sierra_program: &'a SierraProgram,
    name_suffix: &'a str,
) -> Result<&'a Function, RunnerError> {
    sierra_program
        .funcs
        .iter()
        .find(|f| {
            if let Some(name) = &f.id.debug_name {
                name.ends_with(name_suffix)
            } else {
                false
            }
        })
        .ok_or_else(|| RunnerError::MissingMain)
}

/// Creates a list of instructions that will be appended to the program's bytecode.
fn create_code_footer() -> Vec<Instruction> {
    casm! {
        // Add a `ret` instruction used in libfuncs that retrieve the current value of the `fp`
        // and `pc` registers.
        ret;
    }
    .instructions
}

// Create proof_mode specific instructions
// Including the "canonical" proof mode instructions (the ones added by the compiler in cairo 0)
// wich call the firt program instruction and then initiate an infinite loop.
// And also appending the return values to the output builtin's memory segment
fn create_proof_mode_header(builtin_count: i16, return_type_size: i16) -> Vec<Instruction> {
    // As the output builtin is not used by cairo 1 (we forced it for this purpose), it's segment is always empty
    // so we can start writing values directly from it's base, which is located relative to the fp before the other builtin's bases
    let output_fp_offset: i16 = -(builtin_count + 2); // The 2 here represents the return_fp & end segments

    // The pc offset where the original program should start
    // Without this header it should start at 0, but we add 2 for each call and jump instruction (as both of them use immediate values)
    // and also 1 for each instruction added to copy each return value into the output segment
    let program_start_offset: i16 = 4 + return_type_size;

    let mut ctx = casm! {};
    casm_extend! {ctx,
        call rel program_start_offset; // Begin program execution by calling the first instruction in the original program
    };
    // Append each return value to the output segment
    for (i, j) in (1..return_type_size + 1).rev().enumerate() {
        casm_extend! {ctx,
            // [ap -j] is where each return value is located in memory
            // [[fp + output_fp_offet] + 0] is the base of the output segment
            [ap - j] = [[fp + output_fp_offset] + i as i16];
        };
    }
    casm_extend! {ctx,
        jmp rel 0; // Infinite loop
    };
    ctx.instructions
}

/// Returns the instructions to add to the beginning of the code to successfully call the main
/// function, as well as the builtins required to execute the program.
fn create_entry_code(
    sierra_program_registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    casm_program: &CairoProgram,
    type_sizes: &UnorderedHashMap<ConcreteTypeId, i16>,
    func: &Function,
    initial_gas: usize,
    proof_mode: bool,
    args: &[FuncArg],
) -> Result<(Vec<Instruction>, Vec<BuiltinName>), Error> {
    let mut ctx = casm! {};
    // The builtins in the formatting expected by the runner.
    let (builtins, builtin_offset) = get_function_builtins(func, proof_mode);

    // Load all vecs to memory.
    // Load all array args content to memory.
    let mut array_args_data = vec![];
    let mut ap_offset: i16 = 0;
    for arg in args {
        let FuncArg::Array(values) = arg else {
            continue;
        };
        array_args_data.push(ap_offset);
        casm_extend! {ctx,
            %{ memory[ap + 0] = segments.add() %}
            ap += 1;
        }
        for (i, v) in values.iter().enumerate() {
            let arr_at = (i + 1) as i16;
            casm_extend! {ctx,
                [ap + 0] = (v.to_bigint());
                [ap + 0] = [[ap - arr_at] + (i as i16)], ap++;
            };
        }
        ap_offset += (1 + values.len()) as i16;
    }
    let mut array_args_data_iter = array_args_data.iter();
    let after_arrays_data_offset = ap_offset;
    let mut arg_iter = args.iter().enumerate();
    let mut param_index = 0;
    let mut expected_arguments_size = 0;
    if func.signature.param_types.iter().any(|ty| {
        get_info(sierra_program_registry, ty)
            .map(|x| x.long_id.generic_id == SegmentArenaType::ID)
            .unwrap_or_default()
    }) {
        casm_extend! {ctx,
            // SegmentArena segment.
            %{ memory[ap + 0] = segments.add() %}
            // Infos segment.
            %{ memory[ap + 1] = segments.add() %}
            ap += 2;
            [ap + 0] = 0, ap++;
            // Write Infos segment, n_constructed (0), and n_destructed (0) to the segment.
            [ap - 2] = [[ap - 3]];
            [ap - 1] = [[ap - 3] + 1];
            [ap - 1] = [[ap - 3] + 2];
        }
        ap_offset += 3;
    }
    for ty in func.signature.param_types.iter() {
        let info = get_info(sierra_program_registry, ty)
            .ok_or_else(|| Error::NoInfoForType(ty.clone()))?;
        let generic_ty = &info.long_id.generic_id;
        if let Some(offset) = builtin_offset.get(generic_ty) {
            let mut offset = *offset;
            if proof_mode {
                // Everything is off by 2 due to the proof mode header
                offset += 2;
            }
            casm_extend! {ctx,
                [ap + 0] = [fp - offset], ap++;
            }
            ap_offset += 1;
        } else if generic_ty == &SystemType::ID {
            casm_extend! {ctx,
                %{ memory[ap + 0] = segments.add() %}
                ap += 1;
            }
            ap_offset += 1;
        } else if generic_ty == &GasBuiltinType::ID {
            casm_extend! {ctx,
                [ap + 0] = initial_gas, ap++;
            }
            ap_offset += 1;
        } else if generic_ty == &SegmentArenaType::ID {
            let offset = -ap_offset + after_arrays_data_offset;
            casm_extend! {ctx,
                [ap + 0] = [ap + offset] + 3, ap++;
            }
            ap_offset += 1;
        } else {
            let ty_size = type_sizes[ty];
            let param_ap_offset_end = ap_offset + ty_size;
            expected_arguments_size += ty_size;
            while ap_offset < param_ap_offset_end {
                let Some((arg_index, arg)) = arg_iter.next() else {
                    break;
                };
                match arg {
                    FuncArg::Single(value) => {
                        casm_extend! {ctx,
                            [ap + 0] = (value.to_bigint()), ap++;
                        }
                        ap_offset += 1;
                    }
                    FuncArg::Array(values) => {
                        let offset = -ap_offset + array_args_data_iter.next().unwrap();
                        casm_extend! {ctx,
                            [ap + 0] = [ap + (offset)], ap++;
                            [ap + 0] = [ap - 1] + (values.len()), ap++;
                        }
                        ap_offset += 2;
                        if ap_offset > param_ap_offset_end {
                            return Err(Error::ArgumentUnaligned {
                                param_index,
                                arg_index,
                            });
                        }
                    }
                }
            }
            param_index += 1;
        };
    }
    let actual_args_size = args
        .iter()
        .map(|arg| match arg {
            FuncArg::Single(_) => 1,
            FuncArg::Array(_) => 2,
        })
        .sum::<i16>();
    if expected_arguments_size != actual_args_size {
        return Err(Error::ArgumentsSizeMismatch {
            expected: expected_arguments_size,
            actual: actual_args_size,
        });
    }

    let before_final_call = ctx.current_code_offset;
    let final_call_size = 3;
    let offset = final_call_size
        + casm_program.debug_info.sierra_statement_info[func.entry_point.0].code_offset;

    casm_extend! {ctx,
        call rel offset;
        ret;
    }
    assert_eq!(before_final_call + final_call_size, ctx.current_code_offset);

    Ok((ctx.instructions, builtins))
}

fn get_info<'a>(
    sierra_program_registry: &'a ProgramRegistry<CoreType, CoreLibfunc>,
    ty: &'a cairo_lang_sierra::ids::ConcreteTypeId,
) -> Option<&'a cairo_lang_sierra::extensions::types::TypeInfo> {
    sierra_program_registry
        .get_type(ty)
        .ok()
        .map(|ctc| ctc.info())
}

/// Creates the metadata required for a Sierra program lowering to casm.
fn create_metadata(
    sierra_program: &cairo_lang_sierra::program::Program,
    metadata_config: Option<MetadataComputationConfig>,
) -> Result<Metadata, VirtualMachineError> {
    if let Some(metadata_config) = metadata_config {
        calc_metadata(sierra_program, metadata_config).map_err(|err| match err {
            MetadataError::ApChangeError(_) => VirtualMachineError::Unexpected,
            MetadataError::CostError(_) => VirtualMachineError::Unexpected,
        })
    } else {
        Ok(Metadata {
            ap_change_info: calc_ap_changes(sierra_program, |_, _| 0)
                .map_err(|_| VirtualMachineError::Unexpected)?,
            gas_info: GasInfo {
                variable_values: Default::default(),
                function_costs: Default::default(),
            },
        })
    }
}

/// Type representing the Output builtin.
#[derive(Default)]
pub struct OutputType {}
impl cairo_lang_sierra::extensions::NoGenericArgsGenericType for OutputType {
    const ID: cairo_lang_sierra::ids::GenericTypeId =
        cairo_lang_sierra::ids::GenericTypeId::new_inline("Output");
    const STORABLE: bool = true;
    const DUPLICATABLE: bool = false;
    const DROPPABLE: bool = false;
    const ZERO_SIZED: bool = false;
}

fn get_function_builtins(
    func: &Function,
    proof_mode: bool,
) -> (
    Vec<BuiltinName>,
    HashMap<cairo_lang_sierra::ids::GenericTypeId, i16>,
) {
    let entry_params = &func.signature.param_types;
    let mut builtins = Vec::new();
    let mut builtin_offset: HashMap<cairo_lang_sierra::ids::GenericTypeId, i16> = HashMap::new();
    let mut current_offset = 3;
    // Fetch builtins from the entry_params in the standard order
    if entry_params
        .iter()
        .any(|ti| ti.debug_name == Some("Poseidon".into()))
    {
        builtins.push(BuiltinName::poseidon);
        builtin_offset.insert(PoseidonType::ID, current_offset);
        current_offset += 1;
    }
    if entry_params
        .iter()
        .any(|ti| ti.debug_name == Some("EcOp".into()))
    {
        builtins.push(BuiltinName::ec_op);
        builtin_offset.insert(EcOpType::ID, current_offset);
        current_offset += 1
    }
    if entry_params
        .iter()
        .any(|ti| ti.debug_name == Some("Bitwise".into()))
    {
        builtins.push(BuiltinName::bitwise);
        builtin_offset.insert(BitwiseType::ID, current_offset);
        current_offset += 1;
    }
    if entry_params
        .iter()
        .any(|ti| ti.debug_name == Some("RangeCheck".into()))
    {
        builtins.push(BuiltinName::range_check);
        builtin_offset.insert(RangeCheckType::ID, current_offset);
        current_offset += 1;
    }
    if entry_params
        .iter()
        .any(|ti| ti.debug_name == Some("Pedersen".into()))
    {
        builtins.push(BuiltinName::pedersen);
        builtin_offset.insert(PedersenType::ID, current_offset);
        current_offset += 1;
    }
    // Force an output builtin so that we can write the program output into it's segment
    if proof_mode {
        builtins.push(BuiltinName::output);
        builtin_offset.insert(OutputType::ID, current_offset);
    }
    builtins.reverse();
    (builtins, builtin_offset)
}

fn fetch_return_values(
    return_type_size: i16,
    return_type_id: &ConcreteTypeId,
    vm: &VirtualMachine,
) -> Result<Vec<MaybeRelocatable>, Error> {
    let mut return_values = vm.get_return_values(return_type_size as usize)?;
    // Check if this result is a Panic result
    if return_type_id
        .debug_name
        .as_ref()
        .ok_or_else(|| Error::TypeIdNoDebugName(return_type_id.clone()))?
        .starts_with("core::panics::PanicResult::")
    {
        // Check the failure flag (aka first return value)
        if return_values.first() != Some(&MaybeRelocatable::from(0)) {
            // In case of failure, extract the error from the return values (aka last two values)
            let panic_data_end = return_values
                .last()
                .ok_or(Error::FailedToExtractReturnValues)?
                .get_relocatable()
                .ok_or(Error::FailedToExtractReturnValues)?;
            let panic_data_start = return_values
                .get(return_values.len() - 2)
                .ok_or(Error::FailedToExtractReturnValues)?
                .get_relocatable()
                .ok_or(Error::FailedToExtractReturnValues)?;
            let panic_data = vm.get_integer_range(
                panic_data_start,
                (panic_data_end - panic_data_start).map_err(VirtualMachineError::Math)?,
            )?;
            return Err(Error::RunPanic(
                panic_data.iter().map(|c| *c.as_ref()).collect(),
            ));
        } else {
            if return_values.len() < 3 {
                return Err(Error::FailedToExtractReturnValues);
            }
            return_values = return_values[2..].to_vec()
        }
    }
    Ok(return_values)
}

// Calculates builtins' final_stack setting each stop_ptr
// Calling this function is a must if either air_public_input or cairo_pie are needed
fn finalize_builtins(
    proof_mode: bool,
    main_ret_types: &[ConcreteTypeId],
    type_sizes: &UnorderedHashMap<ConcreteTypeId, i16>,
    vm: &mut VirtualMachine,
) -> Result<(), Error> {
    // Set stop pointers for builtins so we can obtain the air public input
    // Cairo 1 programs have other return values aside from the used builtin's final pointers, so we need to hand-pick them
    let ret_types_sizes = main_ret_types
        .iter()
        .map(|id| type_sizes.get(id).cloned().unwrap_or_default());
    let ret_types_and_sizes = main_ret_types.iter().zip(ret_types_sizes.clone());

    let full_ret_types_size: i16 = ret_types_sizes.sum();
    let mut stack_pointer = (vm.get_ap() - (full_ret_types_size as usize).saturating_sub(1))
        .map_err(VirtualMachineError::Math)?;

    // Calculate the stack_ptr for each return builtin in the return values
    let mut builtin_name_to_stack_pointer = HashMap::new();
    for (id, size) in ret_types_and_sizes {
        if let Some(ref name) = id.debug_name {
            let builtin_name = match &*name.to_string() {
                "RangeCheck" => RANGE_CHECK_BUILTIN_NAME,
                "Poseidon" => POSEIDON_BUILTIN_NAME,
                "EcOp" => EC_OP_BUILTIN_NAME,
                "Bitwise" => BITWISE_BUILTIN_NAME,
                "Pedersen" => HASH_BUILTIN_NAME,
                "Output" => OUTPUT_BUILTIN_NAME,
                "Ecdsa" => SIGNATURE_BUILTIN_NAME,
                _ => {
                    stack_pointer.offset += size as usize;
                    continue;
                }
            };
            builtin_name_to_stack_pointer.insert(builtin_name, stack_pointer);
        }
        stack_pointer.offset += size as usize;
    }

    // Set stop pointer for each builtin
    vm.builtins_final_stack_from_stack_pointer_dict(&builtin_name_to_stack_pointer, proof_mode)?;
    Ok(())
}

// #[cfg(test)]
// mod tests {
//     use std::path::Path;

//     use super::*;
//     use cairo_lang_compiler::{compile_cairo_project_at_path, CompilerConfig};
//     use cairo_vm::types::relocatable::Relocatable;
//     use rstest::rstest;

//     fn compile_to_sierra(filename: &str) -> SierraProgram {
//         let compiler_config = CompilerConfig {
//             replace_ids: true,
//             ..CompilerConfig::default()
//         };

//         compile_cairo_project_at_path(Path::new(filename), compiler_config).unwrap()
//     }

//     fn main_hash_panic_result(sierra_program: &SierraProgram) -> bool {
//         let main_func = find_function(sierra_program, "::main").unwrap();
//         main_func
//             .signature
//             .ret_types
//             .last()
//             .and_then(|rt| {
//                 rt.debug_name
//                     .as_ref()
//                     .map(|n| n.as_ref().starts_with("core::panics::PanicResult::"))
//             })
//             .unwrap_or_default()
//     }

//     #[rstest]
//     #[case("../cairo_programs/cairo-1-programs/array_append.cairo")]
//     #[case("../cairo_programs/cairo-1-programs/array_get.cairo")]
//     #[case("../cairo_programs/cairo-1-programs/dictionaries.cairo")]
//     #[case("../cairo_programs/cairo-1-programs/enum_flow.cairo")]
//     #[case("../cairo_programs/cairo-1-programs/enum_match.cairo")]
//     #[case("../cairo_programs/cairo-1-programs/factorial.cairo")]
//     #[case("../cairo_programs/cairo-1-programs/fibonacci.cairo")]
//     #[case("../cairo_programs/cairo-1-programs/hello.cairo")]
//     #[case("../cairo_programs/cairo-1-programs/pedersen_example.cairo")]
//     #[case("../cairo_programs/cairo-1-programs/poseidon.cairo")]
//     #[case("../cairo_programs/cairo-1-programs/print.cairo")]
//     #[case("../cairo_programs/cairo-1-programs/array_append.cairo")]
//     #[case("../cairo_programs/cairo-1-programs/recursion.cairo")]
//     #[case("../cairo_programs/cairo-1-programs/sample.cairo")]
//     #[case("../cairo_programs/cairo-1-programs/simple_struct.cairo")]
//     #[case("../cairo_programs/cairo-1-programs/simple.cairo")]
//     #[case("../cairo_programs/cairo-1-programs/struct_span_return.cairo")]
//     fn check_append_ret_values_to_output_segment(#[case] filename: &str) {
//         // Compile to sierra
//         let sierra_program = compile_to_sierra(filename);
//         // Set proof_mode
//         let cairo_run_config = Cairo1RunConfig {
//             proof_mode: true,
//             layout: "all_cairo",
//             ..Default::default()
//         };
//         // Run program
//         let (_, vm, return_values) = cairo_run_program(&sierra_program, cairo_run_config).unwrap();
//         // When the return type is a PanicResult, we remove the panic wrapper when returning the ret values
//         // And handle the panics returning an error, so we need to add it here
//         let return_values = if main_hash_panic_result(&sierra_program) {
//             let mut rv = vec![Felt252::ZERO.into(), Felt252::ZERO.into()];
//             rv.extend_from_slice(&return_values);
//             rv
//         } else {
//             return_values
//         };
//         // Check that the output segment contains the return values
//         // The output builtin will always be the first builtin, so we know it's segment is 2
//         let output_builtin_segment = vm
//             .get_continuous_range((2, 0).into(), return_values.len())
//             .unwrap();
//         assert_eq!(output_builtin_segment, return_values, "{}", filename);
//         // Just for consistency, we will check that there are no values in the output segment after the return values
//         assert!(vm
//             .get_maybe(&Relocatable::from((2_isize, return_values.len())))
//             .is_none());
//     }
// }
