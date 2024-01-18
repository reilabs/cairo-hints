use std::env;
use std::fs;

use anyhow::{bail, ensure, Context, Result};
use cairo_lang_runner::Arg;
use cairo_lang_runner::CairoHintProcessor;
use cairo_lang_runner::RunnerError;
use cairo_lang_runner::build_hints_dict;
use cairo_lang_runner::short_string::as_cairo_short_string;
use cairo_lang_runner::{RunResultStarknet, RunResultValue, SierraCasmRunner, StarknetState};
use cairo_lang_sierra::program::Function;
use cairo_lang_sierra::program::VersionedProgram;
use cairo_lang_sierra_to_casm::compiler::CairoProgram;
use cairo_lang_sierra_to_casm::metadata::MetadataComputationConfig;
use cairo_lang_sierra_to_casm::metadata::MetadataError;
use cairo_lang_sierra_to_casm::metadata::calc_metadata;
use cairo_lang_sierra_to_casm::metadata::calc_metadata_ap_change_only;
use cairo_oracle_hint_processor::RpcHintProcessor;
use cairo_vm::vm::runners::cairo_runner::RunResources;
use camino::Utf8PathBuf;
use clap::Parser;
use indoc::formatdoc;
use serde::Serializer;
use itertools::chain;

use scarb_metadata::{Metadata, MetadataCommand, ScarbCommand};
use scarb_ui::args::PackagesFilter;
use scarb_ui::components::Status;
use scarb_ui::{Message, OutputFormat, Ui, Verbosity};

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
    
}

fn main() -> Result<()> {
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
    let metadata = create_metadata(&sierra_program.program, metadata_config.clone())?;
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
    let mut hint_processor = RpcHintProcessor::new(cairo_hint_processor, oracle_server);

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
fn create_metadata(
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
