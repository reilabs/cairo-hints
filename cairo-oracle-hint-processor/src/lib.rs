use std::io;
use std::io::Write;
use std::path::PathBuf;

use anyhow::Result;
use bincode::enc::write::Writer;
use cairo_lang_sierra::ids::ConcreteTypeId;
use cairo_lang_sierra::program::Program as SierraProgram;
use cairo_lang_sierra::program_registry::ProgramRegistryError;
use cairo_lang_sierra_to_casm::compiler::CompilationError;
use cairo_lang_sierra_to_casm::metadata::MetadataError;
use cairo_proto_serde::configuration::Configuration;
use cairo_run::Cairo1RunConfig;
use cairo_vm::air_public_input::PublicInputError;
use cairo_vm::cairo_run::EncodeTraceError;
use cairo_vm::types::errors::program_errors::ProgramError;
use cairo_vm::types::relocatable::MaybeRelocatable;
use cairo_vm::vm::errors::memory_errors::MemoryError;
use cairo_vm::vm::errors::runner_errors::RunnerError;
use cairo_vm::vm::errors::trace_errors::TraceError;
use cairo_vm::vm::errors::vm_errors::VirtualMachineError;
use cairo_vm::Felt252;
use thiserror::Error;

mod cairo_run;
pub mod rpc_hint_processor;

mod hint_processor_utils;

// /// Creates the metadata required for a Sierra program lowering to casm.
// fn create_metadata(
//     sierra_program: &cairo_lang_sierra::program::Program,
//     metadata_config: Option<MetadataComputationConfig>,
// ) -> Result<cairo_lang_sierra_to_casm::metadata::Metadata, RunnerError> {
//     if let Some(metadata_config) = metadata_config {
//         calc_metadata(sierra_program, metadata_config)
//     } else {
//         calc_metadata_ap_change_only(sierra_program)
//     }
//     .map_err(|err| match err {
//         MetadataError::ApChangeError(err) => RunnerError::ApChangeError(err),
//         MetadataError::CostError(_) => RunnerError::FailedGasCalculation,
//     })
// }

// /// Creates the metadata required for a Sierra program lowering to casm.
// fn create_metadata(
//     sierra_program: &cairo_lang_sierra::program::Program,
//     metadata_config: Option<MetadataComputationConfig>,
// ) -> Result<cairo_lang_sierra_to_casm::metadata::Metadata, VirtualMachineError> {
//     if let Some(metadata_config) = metadata_config {
//         calc_metadata(sierra_program, metadata_config).map_err(|err| match err {
//             MetadataError::ApChangeError(_) => VirtualMachineError::Unexpected,
//             MetadataError::CostError(_) => VirtualMachineError::Unexpected,
//         })
//     } else {
//         Ok(cairo_lang_sierra_to_casm::metadata::Metadata {
//             ap_change_info: calc_ap_changes(sierra_program, |_, _| 0)
//                 .map_err(|_| VirtualMachineError::Unexpected)?,
//             gas_info: GasInfo {
//                 variable_values: Default::default(),
//                 function_costs: Default::default(),
//             },
//         })
//     }
// }

#[derive(Debug, Error)]
pub enum Error {
    #[error("Invalid arguments")]
    Cli(#[from] clap::Error),
    #[error("Failed to interact with the file system")]
    IO(#[from] std::io::Error),
    #[error(transparent)]
    EncodeTrace(#[from] EncodeTraceError),
    #[error(transparent)]
    VirtualMachine(#[from] VirtualMachineError),
    #[error(transparent)]
    Trace(#[from] TraceError),
    #[error(transparent)]
    PublicInput(#[from] PublicInputError),
    #[error(transparent)]
    Runner(#[from] RunnerError),
    #[error(transparent)]
    ProgramRegistry(#[from] Box<ProgramRegistryError>),
    #[error(transparent)]
    Compilation(#[from] Box<CompilationError>),
    #[error("Failed to compile to sierra:\n {0}")]
    SierraCompilation(String),
    #[error(transparent)]
    Metadata(#[from] MetadataError),
    #[error(transparent)]
    Program(#[from] ProgramError),
    #[error(transparent)]
    Memory(#[from] MemoryError),
    #[error("Program panicked with {0:?}")]
    RunPanic(Vec<Felt252>),
    #[error("Function signature has no return types")]
    NoRetTypesInSignature,
    #[error("No size for concrete type id: {0}")]
    NoTypeSizeForId(ConcreteTypeId),
    #[error("Concrete type id has no debug name: {0}")]
    TypeIdNoDebugName(ConcreteTypeId),
    #[error("No info in sierra program registry for concrete type id: {0}")]
    NoInfoForType(ConcreteTypeId),
    #[error("Failed to extract return values from VM")]
    FailedToExtractReturnValues,
    #[error("Function expects arguments of size {expected} and received {actual} instead.")]
    ArgumentsSizeMismatch { expected: i16, actual: i16 },
    #[error("Function param {param_index} only partially contains argument {arg_index}.")]
    ArgumentUnaligned {
        param_index: usize,
        arg_index: usize,
    },
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
enum FuncArg {
    Array(Vec<Felt252>),
    Single(Felt252),
}

#[derive(Debug, Clone, Default)]
struct FuncArgs(Vec<FuncArg>);

// fn process_args(value: &str) -> Result<FuncArgs, String> {
//     if value.is_empty() {
//         return Ok(FuncArgs::default());
//     }
//     let mut args = Vec::new();
//     let mut input = value.split(' ');
//     while let Some(value) = input.next() {
//         // First argument in an array
//         if value.starts_with('[') {
//             let mut array_arg =
//                 vec![Felt252::from_dec_str(value.strip_prefix('[').unwrap()).unwrap()];
//             // Process following args in array
//             let mut array_end = false;
//             while !array_end {
//                 if let Some(value) = input.next() {
//                     // Last arg in array
//                     if value.ends_with(']') {
//                         array_arg
//                             .push(Felt252::from_dec_str(value.strip_suffix(']').unwrap()).unwrap());
//                         array_end = true;
//                     } else {
//                         array_arg.push(Felt252::from_dec_str(value).unwrap())
//                     }
//                 }
//             }
//             // Finalize array
//             args.push(FuncArg::Array(array_arg))
//         } else {
//             // Single argument
//             args.push(FuncArg::Single(Felt252::from_dec_str(value).unwrap()))
//         }
//     }
//     Ok(FuncArgs(args))
// }

pub struct FileWriter {
    buf_writer: io::BufWriter<std::fs::File>,
    bytes_written: usize,
}

impl Writer for FileWriter {
    fn write(&mut self, bytes: &[u8]) -> Result<(), bincode::error::EncodeError> {
        self.buf_writer
            .write_all(bytes)
            .map_err(|e| bincode::error::EncodeError::Io {
                inner: e,
                index: self.bytes_written,
            })?;

        self.bytes_written += bytes.len();

        Ok(())
    }
}

impl FileWriter {
    fn new(buf_writer: io::BufWriter<std::fs::File>) -> Self {
        Self {
            buf_writer,
            bytes_written: 0,
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        self.buf_writer.flush()
    }
}

pub fn run_1(
    service_config: &Configuration,
    oracle_server: &Option<String>,
    layout: &str,
    trace_file: &Option<PathBuf>,
    memory_file: &Option<PathBuf>,
    sierra_program: &SierraProgram,
    entry_func_name: &str,
    proof_mode: bool,
) -> Result<Vec<MaybeRelocatable>, Error> {
    // let compiler_config = CompilerConfig {
    //     replace_ids: true,
    //     ..CompilerConfig::default()
    // };

    // let sierra_program = (*compile_cairo_project_at_path(&args.filename, compiler_config)
    //     .map_err(|err| Error::SierraCompilation(err.to_string()))?)
    // .clone();

    let cairo_run_config = Cairo1RunConfig {
        proof_mode: proof_mode,
        relocate_mem: memory_file.is_some(), //|| air_public_input.is_some(),
        layout: layout,
        trace_enabled: trace_file.is_some(), //|| args.air_public_input.is_some(),
        args: &[],
        finalize_builtins: false, //args.air_private_input.is_some() || args.cairo_pie_output.is_some(),
    };

    let (runner, _vm, return_values) = cairo_run::cairo_run_program(
        &sierra_program,
        cairo_run_config,
        service_config,
        oracle_server,
        entry_func_name,
    )?;

    // let output_string = if args.print_output {
    //     Some(serialize_output(&vm, &return_values))
    // } else {
    //     None
    // };

    // if let Some(file_path) = args.air_public_input {
    //     let json = runner.get_air_public_input(&vm)?.serialize_json()?;
    //     std::fs::write(file_path, json)?;
    // }

    // if let (Some(trace_file), Some(memory_file)) = (trace_file.clone(), memory_file.clone()) {
    //     // Get absolute paths of trace_file & memory_file
    //     let trace_path = trace_file
    //         .as_path()
    //         .canonicalize()
    //         .unwrap_or(trace_file.clone())
    //         .to_string_lossy()
    //         .to_string();
    //     let memory_path = memory_file
    //         .as_path()
    //         .canonicalize()
    //         .unwrap_or(memory_file.clone())
    //         .to_string_lossy()
    //         .to_string();

    //     // let json = runner
    //     //     .get_air_private_input(&vm)
    //     //     .to_serializable(trace_path, memory_path)
    //     //     .serialize_json()
    //     //     .map_err(PublicInputError::Serde)?;
    //     // std::fs::write(file_path, json)?;
    // }

    // if let Some(ref file_path) = args.cairo_pie_output {
    //     runner.get_cairo_pie(&vm)?.write_zip_file(file_path)?
    // }

    if let Some(trace_path) = trace_file {
        let relocated_trace = runner
            .relocated_trace
            .ok_or(Error::Trace(TraceError::TraceNotRelocated))?;
        let trace_file = std::fs::File::create(trace_path)?;
        let mut trace_writer =
            FileWriter::new(io::BufWriter::with_capacity(3 * 1024 * 1024, trace_file));

        cairo_vm::cairo_run::write_encoded_trace(&relocated_trace, &mut trace_writer)?;
        trace_writer.flush()?;
    }
    if let Some(memory_path) = memory_file {
        let memory_file = std::fs::File::create(memory_path)?;
        let mut memory_writer =
            FileWriter::new(io::BufWriter::with_capacity(5 * 1024 * 1024, memory_file));

        cairo_vm::cairo_run::write_encoded_memory(&runner.relocated_memory, &mut memory_writer)?;
        memory_writer.flush()?;
    }

    Ok(return_values)
}

// fn additional_initialization(vm: &mut VirtualMachine, data_len: usize) -> Result<(), Error> {
//     // Create the builtin cost segment
//     let builtin_cost_segment = vm.add_memory_segment();
//     for token_type in CostTokenType::iter_precost() {
//         vm.insert_value(
//             (builtin_cost_segment + (token_type.offset_in_builtin_costs() as usize))
//                 .map_err(VirtualMachineError::Math)?,
//             Felt252::default(),
//         )?
//     }
//     // Put a pointer to the builtin cost segment at the end of the program (after the
//     // additional `ret` statement).
//     vm.insert_value(
//         (vm.get_pc() + data_len).map_err(VirtualMachineError::Math)?,
//         builtin_cost_segment,
//     )?;

//     Ok(())
// }

// #[allow(clippy::type_complexity)]
// fn build_hints_vec<'b>(
//     instructions: impl Iterator<Item = &'b Instruction>,
// ) -> (Vec<(usize, Vec<Hint>)>, HashMap<usize, Vec<HintParams>>) {
//     let mut hints: Vec<(usize, Vec<Hint>)> = Vec::new();
//     let mut program_hints: HashMap<usize, Vec<HintParams>> = HashMap::new();

//     let mut hint_offset = 0;

//     for instruction in instructions {
//         if !instruction.hints.is_empty() {
//             hints.push((hint_offset, instruction.hints.clone()));
//             program_hints.insert(
//                 hint_offset,
//                 vec![HintParams {
//                     code: hint_offset.to_string(),
//                     accessible_scopes: Vec::new(),
//                     flow_tracking_data: FlowTrackingData {
//                         ap_tracking: ApTracking::default(),
//                         reference_ids: HashMap::new(),
//                     },
//                 }],
//             );
//         }
//         hint_offset += instruction.body.op_size();
//     }
//     (hints, program_hints)
// }

// /// Finds first function ending with `name_suffix`.
// fn find_function<'a>(
//     sierra_program: &'a SierraProgram,
//     name_suffix: &'a str,
// ) -> Result<&'a Function, VMError> {
//     sierra_program
//         .funcs
//         .iter()
//         .find(|f| {
//             if let Some(name) = &f.id.debug_name {
//                 name.ends_with(name_suffix)
//             } else {
//                 false
//             }
//         })
//         .ok_or_else(|| VMError::MissingMain)
// }

// /// Creates a list of instructions that will be appended to the program's bytecode.
// fn create_code_footer() -> Vec<Instruction> {
//     casm! {
//         // Add a `ret` instruction used in libfuncs that retrieve the current value of the `fp`
//         // and `pc` registers.
//         ret;
//     }
//     .instructions
// }

// /// Returns the instructions to add to the beginning of the code to successfully call the main
// /// function, as well as the builtins required to execute the program.
// fn create_entry_code(
//     sierra_program_registry: &ProgramRegistry<CoreType, CoreLibfunc>,
//     casm_program: &CairoProgram,
//     type_sizes: &UnorderedHashMap<ConcreteTypeId, i16>,
//     func: &Function,
//     initial_gas: usize,
//     proof_mode: bool,
//     args: &Vec<FuncArg>,
// ) -> Result<(Vec<Instruction>, Vec<BuiltinName>), Error> {
//     let mut ctx = casm! {};
//     // The builtins in the formatting expected by the runner.
//     let (builtins, builtin_offset) = get_function_builtins(func);
//     // Load all vecs to memory.
//     // Load all array args content to memory.
//     let mut array_args_data = vec![];
//     let mut ap_offset: i16 = 0;
//     for arg in args {
//         let FuncArg::Array(values) = arg else {
//             continue;
//         };
//         array_args_data.push(ap_offset);
//         casm_extend! {ctx,
//             %{ memory[ap + 0] = segments.add() %}
//             ap += 1;
//         }
//         for (i, v) in values.iter().enumerate() {
//             let arr_at = (i + 1) as i16;
//             casm_extend! {ctx,
//                 [ap + 0] = (v.to_bigint());
//                 [ap + 0] = [[ap - arr_at] + (i as i16)], ap++;
//             };
//         }
//         ap_offset += (1 + values.len()) as i16;
//     }
//     let mut array_args_data_iter = array_args_data.iter();
//     let after_arrays_data_offset = ap_offset;
//     let mut arg_iter = args.iter().enumerate();
//     let mut param_index = 0;
//     let mut expected_arguments_size = 0;
//     if func.signature.param_types.iter().any(|ty| {
//         get_info(sierra_program_registry, ty)
//             .map(|x| x.long_id.generic_id == SegmentArenaType::ID)
//             .unwrap_or_default()
//     }) {
//         casm_extend! {ctx,
//             // SegmentArena segment.
//             %{ memory[ap + 0] = segments.add() %}
//             // Infos segment.
//             %{ memory[ap + 1] = segments.add() %}
//             ap += 2;
//             [ap + 0] = 0, ap++;
//             // Write Infos segment, n_constructed (0), and n_destructed (0) to the segment.
//             [ap - 2] = [[ap - 3]];
//             [ap - 1] = [[ap - 3] + 1];
//             [ap - 1] = [[ap - 3] + 2];
//         }
//         ap_offset += 3;
//     }
//     for ty in func.signature.param_types.iter() {
//         let info = get_info(sierra_program_registry, ty)
//             .ok_or_else(|| Error::NoInfoForType(ty.clone()))?;
//         let generic_ty = &info.long_id.generic_id;
//         if let Some(offset) = builtin_offset.get(generic_ty) {
//             let mut offset = *offset;
//             if proof_mode {
//                 // Everything is off by 2 due to the proof mode header
//                 offset += 2;
//             }
//             casm_extend! {ctx,
//                 [ap + 0] = [fp - offset], ap++;
//             }
//             ap_offset += 1;
//         } else if generic_ty == &SystemType::ID {
//             casm_extend! {ctx,
//                 %{ memory[ap + 0] = segments.add() %}
//                 ap += 1;
//             }
//             ap_offset += 1;
//         } else if generic_ty == &GasBuiltinType::ID {
//             casm_extend! {ctx,
//                 [ap + 0] = initial_gas, ap++;
//             }
//             ap_offset += 1;
//         } else if generic_ty == &SegmentArenaType::ID {
//             let offset = -ap_offset + after_arrays_data_offset;
//             casm_extend! {ctx,
//                 [ap + 0] = [ap + offset] + 3, ap++;
//             }
//             ap_offset += 1;
//         } else {
//             let ty_size = type_sizes[ty];
//             let param_ap_offset_end = ap_offset + ty_size;
//             expected_arguments_size += ty_size;
//             while ap_offset < param_ap_offset_end {
//                 let Some((arg_index, arg)) = arg_iter.next() else {
//                     break;
//                 };
//                 match arg {
//                     FuncArg::Single(value) => {
//                         casm_extend! {ctx,
//                             [ap + 0] = (value.to_bigint()), ap++;
//                         }
//                         ap_offset += 1;
//                     }
//                     FuncArg::Array(values) => {
//                         let offset = -ap_offset + array_args_data_iter.next().unwrap();
//                         casm_extend! {ctx,
//                             [ap + 0] = [ap + (offset)], ap++;
//                             [ap + 0] = [ap - 1] + (values.len()), ap++;
//                         }
//                         ap_offset += 2;
//                         if ap_offset > param_ap_offset_end {
//                             return Err(Error::ArgumentUnaligned {
//                                 param_index,
//                                 arg_index,
//                             });
//                         }
//                     }
//                 }
//             }
//             param_index += 1;
//         };
//     }
//     let actual_args_size = args
//         .iter()
//         .map(|arg| match arg {
//             FuncArg::Single(_) => 1,
//             FuncArg::Array(_) => 2,
//         })
//         .sum::<i16>();
//     if expected_arguments_size != actual_args_size {
//         return Err(Error::ArgumentsSizeMismatch {
//             expected: expected_arguments_size,
//             actual: actual_args_size,
//         });
//     }
//     let before_final_call = ctx.current_code_offset;
//     let final_call_size = 3;
//     let offset = final_call_size
//         + casm_program.debug_info.sierra_statement_info[func.entry_point.0].code_offset;
//     casm_extend! {ctx,
//         call rel offset;
//         ret;
//     }
//     assert_eq!(before_final_call + final_call_size, ctx.current_code_offset);
//     Ok((ctx.instructions, builtins))
// }

// fn get_info<'a>(
//     sierra_program_registry: &'a ProgramRegistry<CoreType, CoreLibfunc>,
//     ty: &'a cairo_lang_sierra::ids::ConcreteTypeId,
// ) -> Option<&'a cairo_lang_sierra::extensions::types::TypeInfo> {
//     sierra_program_registry
//         .get_type(ty)
//         .ok()
//         .map(|ctc| ctc.info())
// }

// fn get_function_builtins(
//     func: &Function,
// ) -> (
//     Vec<BuiltinName>,
//     HashMap<cairo_lang_sierra::ids::GenericTypeId, i16>,
// ) {
//     let entry_params = &func.signature.param_types;
//     let mut builtins = Vec::new();
//     let mut builtin_offset: HashMap<cairo_lang_sierra::ids::GenericTypeId, i16> = HashMap::new();
//     let mut current_offset = 3;
//     // Fetch builtins from the entry_params in the standard order
//     if entry_params
//         .iter()
//         .any(|ti| ti.debug_name == Some("Poseidon".into()))
//     {
//         builtins.push(BuiltinName::poseidon);
//         builtin_offset.insert(PoseidonType::ID, current_offset);
//         current_offset += 1;
//     }
//     if entry_params
//         .iter()
//         .any(|ti| ti.debug_name == Some("EcOp".into()))
//     {
//         builtins.push(BuiltinName::ec_op);
//         builtin_offset.insert(EcOpType::ID, current_offset);
//         current_offset += 1
//     }
//     if entry_params
//         .iter()
//         .any(|ti| ti.debug_name == Some("Bitwise".into()))
//     {
//         builtins.push(BuiltinName::bitwise);
//         builtin_offset.insert(BitwiseType::ID, current_offset);
//         current_offset += 1;
//     }
//     if entry_params
//         .iter()
//         .any(|ti| ti.debug_name == Some("RangeCheck".into()))
//     {
//         builtins.push(BuiltinName::range_check);
//         builtin_offset.insert(RangeCheckType::ID, current_offset);
//         current_offset += 1;
//     }
//     if entry_params
//         .iter()
//         .any(|ti| ti.debug_name == Some("Pedersen".into()))
//     {
//         builtins.push(BuiltinName::pedersen);
//         builtin_offset.insert(PedersenType::ID, current_offset);
//     }
//     builtins.reverse();
//     (builtins, builtin_offset)
// }
