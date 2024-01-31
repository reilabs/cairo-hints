use std::env;
use std::fs;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

use anyhow::{bail, ensure, Context, Result};
use cairo_felt::Felt252;
use cairo_lang_compiler::{compile_cairo_project_at_path, CompilerConfig};
use cairo_lang_runner::Arg;
use cairo_lang_runner::CairoHintProcessor;
use cairo_lang_runner::RunnerError;
use cairo_lang_runner::build_hints_dict;
use cairo_lang_runner::short_string::as_cairo_short_string;
use cairo_lang_runner::{RunResultStarknet, RunResultValue, SierraCasmRunner, StarknetState};
use cairo_lang_sierra::extensions::range_check::RangeCheckType;
use cairo_lang_sierra::ids::ConcreteTypeId;
use cairo_lang_sierra::program::Function;
use cairo_lang_sierra::program::VersionedProgram;
use cairo_lang_sierra::program_registry::ProgramRegistryError;
use cairo_lang_sierra_to_casm::compiler::{CairoProgram, CompilationError};
use cairo_lang_sierra_to_casm::metadata::MetadataComputationConfig;
use cairo_lang_sierra_to_casm::metadata::MetadataError;
use cairo_lang_sierra_to_casm::metadata::calc_metadata;
use cairo_lang_sierra_to_casm::metadata::calc_metadata_ap_change_only;
use cairo_vm::air_public_input::PublicInputError;
use cairo_vm::cairo_run::EncodeTraceError;
use cairo_vm::serde::deserialize_program::BuiltinName;
use cairo_vm::types::errors::program_errors::ProgramError;
use cairo_vm::types::relocatable::MaybeRelocatable;
use cairo_vm::vm::errors::memory_errors::MemoryError;
use cairo_vm::vm::errors::trace_errors::TraceError;
use cairo_vm::vm::errors::vm_errors::VirtualMachineError;
use cairo_oracle_hint_processor::RpcHintProcessor;
use cairo_vm::vm::runners::cairo_runner::RunResources;
use camino::Utf8PathBuf;
use clap::Parser;
use indoc::formatdoc;
use serde::Serializer;
use itertools::chain;
#![allow(unused_imports)]
use bincode::enc::write::Writer;
use cairo_lang_casm::casm;
use cairo_lang_casm::casm_extend;
use cairo_lang_casm::hints::Hint;
use cairo_lang_casm::instructions::Instruction;
use cairo_lang_compiler::{compile_cairo_project_at_path, CompilerConfig};
use cairo_lang_sierra::extensions::bitwise::BitwiseType;
use cairo_lang_sierra::extensions::core::CoreTypeConcrete;
use cairo_lang_sierra::extensions::core::{CoreLibfunc, CoreType};
use cairo_lang_sierra::extensions::ec::EcOpType;
use cairo_lang_sierra::extensions::gas::GasBuiltinType;
use cairo_lang_sierra::extensions::pedersen::PedersenType;
use cairo_lang_sierra::extensions::poseidon::PoseidonType;
use cairo_lang_sierra::extensions::range_check::RangeCheckType;
use cairo_lang_sierra::extensions::segment_arena::SegmentArenaType;
use cairo_lang_sierra::extensions::starknet::syscalls::SystemType;
use cairo_lang_sierra::extensions::ConcreteType;
use cairo_lang_sierra::extensions::NamedType;
use cairo_lang_sierra::ids::ConcreteTypeId;
use cairo_lang_sierra::program::Function;
use cairo_lang_sierra::program::Program as SierraProgram;
use cairo_lang_sierra::program_registry::{ProgramRegistry, ProgramRegistryError};
use cairo_lang_sierra::{extensions::gas::CostTokenType, ProgramParser};
use cairo_lang_sierra_ap_change::calc_ap_changes;
use cairo_lang_sierra_gas::gas_info::GasInfo;
use cairo_lang_sierra_to_casm::compiler::CairoProgram;
use cairo_lang_sierra_to_casm::compiler::CompilationError;
use cairo_lang_sierra_to_casm::metadata::Metadata;
use cairo_lang_sierra_to_casm::metadata::MetadataComputationConfig;
use cairo_lang_sierra_to_casm::metadata::MetadataError;
use cairo_lang_sierra_to_casm::{compiler::compile, metadata::calc_metadata};
use cairo_lang_sierra_type_size::get_type_size_map;
use cairo_lang_utils::ordered_hash_map::OrderedHashMap;
use cairo_lang_utils::unordered_hash_map::UnorderedHashMap;
use cairo_vm::air_public_input::PublicInputError;
use cairo_vm::cairo_run;
use cairo_vm::cairo_run::EncodeTraceError;
use cairo_vm::hint_processor::cairo_1_hint_processor::hint_processor::Cairo1HintProcessor;
use cairo_vm::hint_processor::rpc_hint_processor::RpcHintProcessor;
use cairo_vm::serde::deserialize_program::BuiltinName;
use cairo_vm::serde::deserialize_program::{ApTracking, FlowTrackingData, HintParams};
use cairo_vm::types::errors::program_errors::ProgramError;
use cairo_vm::types::relocatable::Relocatable;
use cairo_vm::utils::bigint_to_felt;
use cairo_vm::vm::decoding::decoder::decode_instruction;
use cairo_vm::vm::errors::cairo_run_errors::CairoRunError;
use cairo_vm::vm::errors::memory_errors::MemoryError;
use cairo_vm::vm::errors::runner_errors::RunnerError;
use cairo_vm::vm::errors::trace_errors::TraceError;
use cairo_vm::vm::errors::vm_errors::VirtualMachineError;
use cairo_vm::vm::runners::cairo_runner::RunnerMode;
use cairo_vm::{
    serde::deserialize_program::ReferenceManager,
    types::{program::Program, relocatable::MaybeRelocatable},
    vm::{
        runners::cairo_runner::{CairoRunner, RunResources},
        vm_core::VirtualMachine,
    },
    Felt252,
};
use clap::{CommandFactory, Parser, ValueHint};
use itertools::{chain, Itertools};
use std::borrow::Cow;
use std::io::BufWriter;
use std::io::Write;
use std::path::PathBuf;
use std::{collections::HashMap, io, path::Path};
use thiserror::Error;
use scarb_metadata::{Metadata, MetadataCommand, ScarbCommand};
use scarb_ui::args::PackagesFilter;
use scarb_ui::components::Status;
use scarb_ui::{Message, OutputFormat, Ui, Verbosity};
use thiserror::Error;
use cairo_proto_serde::configuration::Configuration;

mod deserialization;

/// Execute the main function of a package.
#[derive(Parser, Clone, Debug)]
#[command(author, version)]
struct Args {
    /// Name of the package.
    #[command(flatten)]
    packages_filter: PackagesFilter,

    /// Maximum amount of gas available to the program.
    #[arg(long)]
    available_gas: Option<usize>,

    /// Print more items in memory.
    #[arg(long, default_value_t = false)]
    print_full_memory: bool,

    /// Do not rebuild the package.
    #[arg(long, default_value_t = false)]
    no_build: bool,

    /// Input to the program.
    #[arg(default_value = "[]")]
    program_input: deserialization::Args,

    /// Oracle server URL.
    #[arg(long)]
    oracle_server: Option<String>,

    #[arg(long)]
    service_config: Option<PathBuf>,
}

fn main_scarb() -> Result<()> {
    let args: Args = Args::parse();
    let available_gas = GasLimit::parse(args.available_gas);

    let ui = Ui::new(Verbosity::default(), OutputFormat::Text);

    let metadata = MetadataCommand::new().inherit_stderr().exec()?;

    let package = args.packages_filter.match_one(&metadata)?;

    if !args.no_build {
        let filter = PackagesFilter::generate_for::<Metadata>(vec![package.clone()].iter());
        ScarbCommand::new()
            .arg("build")
            .env("SCARB_PACKAGES_FILTER", filter.to_env())
            .run()?;
    }

    let filename = format!("{}.sierra.json", package.name);
    let path = Utf8PathBuf::from(env::var("SCARB_TARGET_DIR")?)
        .join(env::var("SCARB_PROFILE")?)
        .join(filename.clone());

    ensure!(
        path.exists(),
        formatdoc! {r#"
            package has not been compiled, file does not exist: {filename}
            help: run `scarb build` to compile the package
        "#}
    );

    ui.print(Status::new("Running", &package.name));

    let sierra_program = serde_json::from_str::<VersionedProgram>(
        &fs::read_to_string(path.clone())
            .with_context(|| format!("failed to read Sierra file: {path}"))?,
    )
    .with_context(|| format!("failed to deserialize Sierra program: {path}"))?
    .into_v1()
    .with_context(|| format!("failed to load Sierra program: {path}"))?;

    if available_gas.is_disabled() && sierra_program.program.requires_gas_counter() {
        bail!("program requires gas counter, please provide `--available-gas` argument");
    }

    let metadata_config = if available_gas.is_disabled() {
        None
    } else {
        Some(Default::default())
    };
    let runner = SierraCasmRunner::new(
        sierra_program.program.clone(),
        metadata_config.clone(),
        Default::default(),
    )?;

    // TODO: this shouldn't be needed. we should call into cairo-lang-runner
    let metadata = create_metadata_scarb(&sierra_program.program, metadata_config.clone())?;
    let casm_program = cairo_lang_sierra_to_casm::compiler::compile(
        &sierra_program.program,
        &metadata,
        metadata_config.is_some(),
    )?;

    let result = run_with_oracle_hint_processor(
            &runner,
            &casm_program,
            runner.find_function("::main")?,
            &args.program_input,
            available_gas.value(),
            StarknetState::default(),
            &args.oracle_server,
            &args.service_config,
        )
        .context("failed to run the function")?;

    ui.print(Summary {
        result,
        print_full_memory: args.print_full_memory,
        gas_defined: available_gas.is_defined(),
    });

    Ok(())
}

/// Runs the vm starting from a function in the context of a given starknet state.
pub fn run_with_oracle_hint_processor(
    runner: &SierraCasmRunner,
    casm_program: &CairoProgram,
    func: &Function,
    args: &[Arg],
    available_gas: Option<usize>,
    starknet_state: StarknetState,
    oracle_server: &Option<String>,
    service_config: &Option<PathBuf>,
) -> Result<RunResultStarknet, RunnerError> {
    let initial_gas = runner.get_initial_available_gas(func, available_gas)?;
    let (entry_code, builtins) = runner.create_entry_code(func, args, initial_gas)?;
    let footer = runner.create_code_footer();
    let instructions =
        chain!(entry_code.iter(), casm_program.instructions.iter(), footer.iter());
    let (hints_dict, string_to_hint) = build_hints_dict(instructions.clone());
    let cairo_hint_processor = CairoHintProcessor {
        runner: Some(runner),
        starknet_state: starknet_state.clone(),
        string_to_hint,
        run_resources: RunResources::default(),
    };

    let service_config = match service_config {
        Some(path) => {
            let file = File::open(path).unwrap();
            let reader = BufReader::new(file);
            serde_json::from_reader(reader).unwrap()
        }
        None => Configuration::default()
    };
    let mut hint_processor = RpcHintProcessor::new(cairo_hint_processor, oracle_server, &service_config);

    runner.run_function(func, &mut hint_processor, hints_dict, instructions, builtins).map(|v| {
        RunResultStarknet {
            gas_counter: v.gas_counter,
            memory: v.memory,
            value: v.value,
            starknet_state: hint_processor.starknet_state(),
        }
    })
}


struct Summary {
    result: RunResultStarknet,
    print_full_memory: bool,
    gas_defined: bool,
}

impl Message for Summary {
    fn print_text(self)
    where
        Self: Sized,
    {
        match self.result.value {
            RunResultValue::Success(values) => {
                println!("Run completed successfully, returning {values:?}")
            }
            RunResultValue::Panic(values) => {
                print!("Run panicked with [");
                for value in &values {
                    match as_cairo_short_string(value) {
                        Some(as_string) => print!("{value} ('{as_string}'), "),
                        None => print!("{value}, "),
                    }
                }
                println!("].")
            }
        }

        if self.gas_defined {
            if let Some(gas) = self.result.gas_counter {
                println!("Remaining gas: {gas}");
            }
        }

        if self.print_full_memory {
            print!("Full memory: [");
            for cell in &self.result.memory {
                match cell {
                    None => print!("_, "),
                    Some(value) => print!("{value}, "),
                }
            }
            println!("]");
        }
    }

    fn structured<S: Serializer>(self, _ser: S) -> Result<S::Ok, S::Error>
    where
        Self: Sized,
    {
        todo!("JSON output is not implemented yet for this command")
    }
}

enum GasLimit {
    Disabled,
    Unlimited,
    Limited(usize),
}
impl GasLimit {
    pub fn parse(value: Option<usize>) -> Self {
        match value {
            Some(0) => GasLimit::Disabled,
            Some(value) => GasLimit::Limited(value),
            None => GasLimit::Unlimited,
        }
    }

    pub fn is_disabled(&self) -> bool {
        matches!(self, GasLimit::Disabled)
    }

    pub fn is_defined(&self) -> bool {
        !matches!(self, GasLimit::Unlimited)
    }

    pub fn value(&self) -> Option<usize> {
        match self {
            GasLimit::Disabled => None,
            GasLimit::Limited(value) => Some(*value),
            GasLimit::Unlimited => Some(usize::MAX),
        }
    }
}


/// Creates the metadata required for a Sierra program lowering to casm.
fn create_metadata_scarb(
    sierra_program: &cairo_lang_sierra::program::Program,
    metadata_config: Option<MetadataComputationConfig>,
) -> Result<cairo_lang_sierra_to_casm::metadata::Metadata, RunnerError> {
    if let Some(metadata_config) = metadata_config {
        calc_metadata(sierra_program, metadata_config)
    } else {
        calc_metadata_ap_change_only(sierra_program)
    }
    .map_err(|err| match err {
        MetadataError::ApChangeError(err) => RunnerError::ApChangeError(err),
        MetadataError::CostError(_) => RunnerError::FailedGasCalculation,
    })
}












fn validate_layout(value: &str) -> Result<String, String> {
    match value {
        "plain"
        | "small"
        | "dex"
        | "starknet"
        | "starknet_with_keccak"
        | "recursive_large_output"
        | "all_cairo"
        | "all_solidity"
        | "dynamic" => Ok(value.to_string()),
        _ => Err(format!("{value} is not a valid layout")),
    }
}

#[derive(Debug, Error)]
enum Error {
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
}

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

fn run(args: impl Iterator<Item = String>) -> Result<Vec<MaybeRelocatable>, Error> {
    let args = Args::try_parse_from(args)?;

    let compiler_config = CompilerConfig {
        replace_ids: true,
        ..CompilerConfig::default()
    };
    let sierra_program = (*compile_cairo_project_at_path(&args.filename, compiler_config)
        .map_err(|err| Error::SierraCompilation(err.to_string()))?)
        .clone();

    let metadata_config = Some(Default::default());

    let gas_usage_check = metadata_config.is_some();
    let metadata = create_metadata(&sierra_program, metadata_config)?;
    let sierra_program_registry = ProgramRegistry::<CoreType, CoreLibfunc>::new(&sierra_program)?;

    let type_sizes =
        get_type_size_map(&sierra_program, &sierra_program_registry).unwrap_or_default();
    let casm_program =
        cairo_lang_sierra_to_casm::compiler::compile(&sierra_program, &metadata, gas_usage_check)?;


    let main_func = find_function(&sierra_program, "::main")?;

    let initial_gas = 9999999999999_usize;

    // Modified entry code to be compatible with custom cairo1 Proof Mode.
    // This adds code that's needed for dictionaries, adjusts ap for builtin pointers, adds initial gas for the gas builtin if needed, and sets up other necessary code for cairo1
    let (entry_code, builtins) = create_entry_code(
        &sierra_program_registry,
        &casm_program,
        &type_sizes,
        main_func,
        initial_gas,
    )?;

    println!("Compiling with proof mode and running ...");

    // This information can be useful for the users using the prover.
    println!("Builtins used: {:?}", builtins);

    // Prepare "canonical" proof mode instructions. These are usually added by the compiler in cairo 0
    let mut ctx = casm! {};
    casm_extend! {ctx,
        call rel 4;
        jmp rel 0;
    };
    let proof_mode_header = ctx.instructions;

    // Get the user program instructions
    let program_instructions = casm_program.instructions.iter();

    // This footer is used by lib funcs
    let libfunc_footer = create_code_footer();

    // This is the program we are actually proving
    // With embedded proof mode, cairo1 header and the libfunc footer
    let instructions = chain!(
        proof_mode_header.iter(),
        entry_code.iter(),
        program_instructions,
        libfunc_footer.iter()
    );

    let (processor_hints, program_hints) = build_hints_vec(instructions.clone());

    let hint_processor = Cairo1HintProcessor::new(&processor_hints, RunResources::default());
    let mut hint_processor = RpcHintProcessor::new(hint_processor, args.oracle_server);

    let data: Vec<MaybeRelocatable> = instructions
        .flat_map(|inst| inst.assemble().encode())
        .map(|x| bigint_to_felt(&x).unwrap_or_default())
        .map(MaybeRelocatable::from)
        .collect();

    let data_len = data.len();

    let starting_pc = 0;

    let program = Program::new_for_proof(
        builtins,
        data,
        starting_pc,
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
    )?;

    let mut runner = CairoRunner::new_v2(&program, &args.layout, RunnerMode::ProofModeCairo1)?;

    let mut vm = VirtualMachine::new(args.trace_file.is_some());
    let end = runner.initialize(&mut vm)?;

    additional_initialization(&mut vm, data_len)?;

    // Run it until the infinite loop
    runner.run_until_pc(end, &mut vm, &mut hint_processor)?;

    // Then pad it to the power of 2
    runner.run_until_next_power_of_2(&mut vm, &mut hint_processor)?;

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
            // In case of failure, extract the error from teh return values (aka last two values)
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

    runner.relocate(&mut vm, true)?;

    if let Some(trace_path) = args.trace_file {
        let relocated_trace = runner
            .relocated_trace
            .ok_or(Error::Trace(TraceError::TraceNotRelocated))?;
        let trace_file = std::fs::File::create(trace_path)?;
        let mut trace_writer =
            FileWriter::new(io::BufWriter::with_capacity(3 * 1024 * 1024, trace_file));

        cairo_run::write_encoded_trace(&relocated_trace, &mut trace_writer)?;
        trace_writer.flush()?;
    }
    if let Some(memory_path) = args.memory_file {
        let memory_file = std::fs::File::create(memory_path)?;
        let mut memory_writer =
            FileWriter::new(io::BufWriter::with_capacity(5 * 1024 * 1024, memory_file));

        cairo_run::write_encoded_memory(&runner.relocated_memory, &mut memory_writer)?;
        memory_writer.flush()?;
    }

    Ok(return_values)
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

fn main() -> Result<(), Error> {
    match run(std::env::args()) {
        Err(Error::Cli(err)) => err.exit(),
        Ok(return_values) => {
            if !return_values.is_empty() {
                let return_values_string_list =
                    return_values.iter().map(|m| m.to_string()).join(", ");
                println!("Return values : [{}]", return_values_string_list);
            }
            Ok(())
        }
        Err(Error::RunPanic(panic_data)) => {
            if !panic_data.is_empty() {
                let panic_data_string_list = panic_data
                    .iter()
                    .map(|m| {
                        // Try to parse to utf8 string
                        let msg = String::from_utf8(m.to_bytes_be().to_vec());
                        if let Ok(msg) = msg {
                            format!("{} ('{}')", m, msg)
                        } else {
                            m.to_string()
                        }
                    })
                    .join(", ");
                println!("Run panicked with: [{}]", panic_data_string_list);
            }
            Ok(())
        }
        Err(err) => Err(err),
    }
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

// #[derive(Debug)]
// enum OracleDef {
//     Literal,
//     Tuple(Vec<OracleDef>),
//     Array(Box<OracleDef>),
//     Unsupported,
// }

// fn build_type_tree<'a>(typ: &CoreTypeConcrete, registry: &ProgramRegistry<CoreType, CoreLibfunc>) -> OracleDef {
//     match typ {
//         CoreTypeConcrete::Array(array_type) => {
//             let x = build_type_tree(registry.get_type(&array_type.ty).unwrap(), registry);
//             OracleDef::Array(Box::new(x))
//         },
//         CoreTypeConcrete::Bitwise(_) => todo!(),
//         CoreTypeConcrete::Box(_) => todo!(),
//         CoreTypeConcrete::EcOp(_) => todo!(),
//         CoreTypeConcrete::EcPoint(_) => todo!(),
//         CoreTypeConcrete::EcState(_) => todo!(),
//         CoreTypeConcrete::Felt252(_) => {
//             OracleDef::Literal
//         }
//         CoreTypeConcrete::GasBuiltin(_) => todo!(),
//         CoreTypeConcrete::BuiltinCosts(_) => todo!(),
//         CoreTypeConcrete::Uint8(_) => todo!(),
//         CoreTypeConcrete::Uint16(_) => todo!(),
//         CoreTypeConcrete::Uint32(_) => todo!(),
//         CoreTypeConcrete::Uint64(_) => todo!(),
//         CoreTypeConcrete::Uint128(_) => todo!(),
//         CoreTypeConcrete::Uint128MulGuarantee(_) => todo!(),
//         CoreTypeConcrete::Sint8(_) => todo!(),
//         CoreTypeConcrete::Sint16(_) => todo!(),
//         CoreTypeConcrete::Sint32(_) => todo!(),
//         CoreTypeConcrete::Sint64(_) => todo!(),
//         CoreTypeConcrete::Sint128(_) => todo!(),
//         CoreTypeConcrete::NonZero(_) => todo!(),
//         CoreTypeConcrete::Nullable(_) => todo!(),
//         CoreTypeConcrete::RangeCheck(_) => todo!(),
//         CoreTypeConcrete::Uninitialized(_) => todo!(),
//         CoreTypeConcrete::Enum(enum_type) => {
//             // TODO: no idea how this would work
//             let x: Vec<OracleDef> = enum_type.variants.iter().map(|ty| {
//                 build_type_tree(registry.get_type(ty).unwrap(), registry)
//             }).collect::<Vec<_>>();
//             OracleDef::Tuple(x)
//         }
//         CoreTypeConcrete::Struct(struct_type) => {
//             let x = struct_type.members.iter().map(|ty| {
//                 println!("member name {ty:?}");
//                 build_type_tree(registry.get_type(ty).unwrap(), registry)
//             }).collect::<Vec<_>>();
//             OracleDef::Tuple(x)
//         },
//         CoreTypeConcrete::Felt252Dict(_) => todo!(),
//         CoreTypeConcrete::Felt252DictEntry(_) => todo!(),
//         CoreTypeConcrete::SquashedFelt252Dict(_) => todo!(),
//         CoreTypeConcrete::Pedersen(_) => todo!(),
//         CoreTypeConcrete::Poseidon(_) => todo!(),
//         CoreTypeConcrete::Span(_) => todo!(),
//         CoreTypeConcrete::StarkNet(_) => todo!(),
//         CoreTypeConcrete::SegmentArena(_) => todo!(),
//         CoreTypeConcrete::Snapshot(_) => todo!(),
//         CoreTypeConcrete::Bytes31(_) => todo!(),
//     }
// }

// fn build_oracles<'a>(
//     sierra_program: &'a SierraProgram,
//     registry: &ProgramRegistry<CoreType, CoreLibfunc>,
// ) -> Result<(), RunnerError> {
//     let instances: Vec<&'a Function> = sierra_program
//         .funcs
//         .iter()
//         .filter(|f| {
//             if let Some(name) = &f.id.debug_name {
//                 name.contains("::oracle::ask_oracle::<")
//             } else {
//                 false
//             }
//         })
//         .collect();

//     for instance in instances {
//         let request_ty = instance.signature.param_types.first().unwrap();
//         let response_ty = instance.signature.ret_types.first().unwrap();

//         let request_def = build_type_tree(registry.get_type(request_ty).unwrap(), registry);
//         let response_def = build_type_tree(registry.get_type(response_ty).unwrap(), registry);

//         println!("ORACLE INSTANCE {request_def:?} -> {response_def:?}");
//     }

//     Ok(())
// }

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

/// Returns the instructions to add to the beginning of the code to successfully call the main
/// function, as well as the builtins required to execute the program.
fn create_entry_code(
    sierra_program_registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    casm_program: &CairoProgram,
    type_sizes: &UnorderedHashMap<ConcreteTypeId, i16>,
    func: &Function,
    initial_gas: usize,
) -> Result<(Vec<Instruction>, Vec<BuiltinName>), Error> {
    let mut ctx = casm! {};
    // The builtins in the formatting expected by the runner.
    let (builtins, builtin_offset) = get_function_builtins(func);
    // Load all vecs to memory.
    let mut ap_offset: i16 = 0;
    let after_vecs_offset = ap_offset;
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
        let ty_size = type_sizes[ty];
        let generic_ty = &info.long_id.generic_id;
        if let Some(offset) = builtin_offset.get(generic_ty) {
            // Everything is off by 2 due to the proof mode header
            let offset = offset + 2;
            casm_extend! {ctx,
                [ap + 0] = [fp - offset], ap++;
            }
        } else if generic_ty == &SystemType::ID {
            casm_extend! {ctx,
                %{ memory[ap + 0] = segments.add() %}
                ap += 1;
            }
        } else if generic_ty == &GasBuiltinType::ID {
            casm_extend! {ctx,
                [ap + 0] = initial_gas, ap++;
            }
        } else if generic_ty == &SegmentArenaType::ID {
            let offset = -ap_offset + after_vecs_offset;
            casm_extend! {ctx,
                [ap + 0] = [ap + offset] + 3, ap++;
            }
            // This code should be re enabled to make the programs work with arguments

            // } else if let Some(Arg::Array(_)) = arg_iter.peek() {
            //     let values = extract_matches!(arg_iter.next().unwrap(), Arg::Array);
            //     let offset = -ap_offset + vecs.pop().unwrap();
            //     expected_arguments_size += 1;
            //     casm_extend! {ctx,
            //         [ap + 0] = [ap + (offset)], ap++;
            //         [ap + 0] = [ap - 1] + (values.len()), ap++;
            //     }
            // } else {
            //     let arg_size = ty_size;
            //     expected_arguments_size += arg_size as usize;
            //     for _ in 0..arg_size {
            //         if let Some(value) = arg_iter.next() {
            //             let value = extract_matches!(value, Arg::Value);
            //             casm_extend! {ctx,
            //                 [ap + 0] = (value.to_bigint()), ap++;
            //             }
            //         }
            //     }
        };
        ap_offset += ty_size;
    }
    // if expected_arguments_size != args.len() {
    //     return Err(RunnerError::ArgumentsSizeMismatch {
    //         expected: expected_arguments_size,
    //         actual: args.len(),
    //     });
    // }

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
        calc_metadata(sierra_program, metadata_config, false).map_err(|err| match err {
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

fn get_function_builtins(
    func: &Function,
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
    }
    builtins.reverse();
    (builtins, builtin_offset)
}