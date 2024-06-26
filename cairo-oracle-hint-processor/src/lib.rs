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
use cairo_vm::types::layout_name::LayoutName;
use cairo_vm::vm::errors::memory_errors::MemoryError;
use cairo_vm::vm::errors::runner_errors::RunnerError;
use cairo_vm::vm::errors::trace_errors::TraceError;
use cairo_vm::vm::errors::vm_errors::VirtualMachineError;
use cairo_vm::Felt252;
use thiserror::Error;

mod cairo_run;
pub mod rpc_hint_processor;

mod hint_processor_utils;

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
    #[error("Configuration error: {0}")]
    ConfigError(String),
    #[error("Servers configuration file error: {0}")]
    ServersConfigFileError(String),
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum FuncArg {
    Array(Vec<Felt252>),
    Single(Felt252),
}

#[derive(Debug, Clone, Default)]
pub struct FuncArgs(pub Vec<FuncArg>);

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
    configuration: &Configuration,
    layout: &LayoutName,
    trace_file: &Option<PathBuf>,
    memory_file: &Option<PathBuf>,
    args: &FuncArgs,
    sierra_program: &SierraProgram,
    entry_func_name: &str,
    proof_mode: bool,
) -> Result<Option<String>, Error> {
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
        layout: *layout,
        trace_enabled: trace_file.is_some(), //|| args.air_public_input.is_some(),
        args: &args.0,
        finalize_builtins: false, //args.air_private_input.is_some() || args.cairo_pie_output.is_some(),
        append_return_values: proof_mode,
        serialize_output: true,
    };

    let (runner, _, serialized_output) = cairo_run::cairo_run_program(
        &sierra_program,
        cairo_run_config,
        configuration,
        entry_func_name,
    )?;

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

    Ok(serialized_output)
}
