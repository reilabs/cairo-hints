use std::path::Path;
use std::sync::Mutex;

use anyhow::{bail, Result};
use cairo_felt::Felt252;
use cairo_lang_compiler::db::RootDatabase;
use cairo_lang_compiler::diagnostics::DiagnosticsReporter;
use cairo_lang_compiler::project::setup_project;
use cairo_lang_filesystem::cfg::{Cfg, CfgSet};
use cairo_lang_filesystem::ids::CrateId;
use cairo_lang_sierra::program::Program;
use cairo_lang_starknet::starknet_plugin_suite;
use cairo_lang_test_plugin::test_config::{PanicExpectation, TestExpectation};
use cairo_lang_test_plugin::{
    compile_test_prepared_db, test_plugin_suite, TestCompilation, TestConfig,
};
use cairo_oracle_hint_processor::{run_1, Error, FuncArgs};
use cairo_proto_serde::configuration::Configuration;
use cairo_vm::types::layout_name::LayoutName;
use cairo_vm::Felt252 as VMFelt;
use colored::Colorize;
use itertools::Itertools;
use rayon::prelude::{IntoParallelIterator, ParallelIterator};

#[cfg(test)]
mod test;

/// Compile and run tests.
pub struct TestRunner {
    compiler: TestCompiler,
    config: TestRunConfig,
}

impl TestRunner {
    /// Configure a new test runner
    ///
    /// # Arguments
    ///
    /// * `path` - The path to compile and run its tests
    /// * `filter` - Run only tests containing the filter string
    /// * `include_ignored` - Include ignored tests as well
    /// * `ignored` - Run ignored tests only
    /// * `starknet` - Add the starknet plugin to run the tests
    pub fn new(path: &Path, starknet: bool, config: TestRunConfig) -> Result<Self> {
        let compiler = TestCompiler::try_new(path, starknet)?;
        Ok(Self { compiler, config })
    }

    /// Runs the tests and process the results for a summary.
    pub fn run(
        &self,
        oracle_server: &Option<String>,
        configuration: &Configuration,
        layout: &LayoutName,
    ) -> Result<Option<TestsSummary>> {
        let runner = CompiledTestRunner::new(self.compiler.build()?, self.config.clone());
        runner.run(oracle_server, configuration, layout)
    }
}

pub struct CompiledTestRunner {
    pub compiled: TestCompilation,
    pub config: TestRunConfig,
}

impl CompiledTestRunner {
    /// Configure a new compiled test runner
    ///
    /// # Arguments
    ///
    /// * `compiled` - The compiled tests to run
    /// * `config` - Test run configuration
    pub fn new(compiled: TestCompilation, config: TestRunConfig) -> Self {
        Self { compiled, config }
    }

    /// Execute preconfigured test execution.
    pub fn run(
        self,
        oracle_server: &Option<String>,
        configuration: &Configuration,
        layout: &LayoutName,
    ) -> Result<Option<TestsSummary>> {
        let (compiled, filtered_out) = filter_test_cases(
            self.compiled,
            self.config.include_ignored,
            self.config.ignored,
            self.config.filter,
        );
        let TestsSummary {
            passed,
            failed,
            ignored,
            failed_run_results,
        } = run_tests(
            compiled.named_tests,
            compiled.sierra_program,
            // compiled.function_set_costs,
            // compiled.contracts_info,
            oracle_server,
            configuration,
            layout,
        )?;

        if failed.is_empty() {
            println!(
                "test result: {}. {} passed; {} failed; {} ignored; {filtered_out} filtered out;",
                "ok".bright_green(),
                passed.len(),
                failed.len(),
                ignored.len()
            );
            Ok(None)
        } else {
            println!("failures:");
            for (failure, run_result) in failed.iter().zip_eq(failed_run_results) {
                print!("   {failure} - ");
                match run_result {
                    RunResultValue::Success(_) => {
                        println!("expected panic but finished successfully.");
                    }
                    RunResultValue::Panic(panic_data) => {
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
                    }
                }
            }
            println!();
            bail!(
                "test result: {}. {} passed; {} failed; {} ignored",
                "FAILED".bright_red(),
                passed.len(),
                failed.len(),
                ignored.len()
            );
        }
    }
}

/// Configuration of compiled tests runner.
#[derive(Clone, Debug)]
pub struct TestRunConfig {
    pub filter: String,
    pub include_ignored: bool,
    pub ignored: bool,
}

/// The test cases compiler.
pub struct TestCompiler {
    pub db: RootDatabase,
    pub main_crate_ids: Vec<CrateId>,
    pub test_crate_ids: Vec<CrateId>,
    pub starknet: bool,
}

impl TestCompiler {
    /// Configure a new test compiler
    ///
    /// # Arguments
    ///
    /// * `path` - The path to compile and run its tests
    /// * `starknet` - Add the starknet plugin to run the tests
    pub fn try_new(path: &Path, starknet: bool) -> Result<Self> {
        let db = &mut {
            let mut b = RootDatabase::builder();
            b.detect_corelib();
            b.with_cfg(CfgSet::from_iter([Cfg::name("test")]));
            b.with_plugin_suite(test_plugin_suite());
            if starknet {
                b.with_plugin_suite(starknet_plugin_suite());
            }

            b.build()?
        };

        let main_crate_ids = setup_project(db, Path::new(&path))?;

        if DiagnosticsReporter::stderr()
            .with_crates(&main_crate_ids)
            .check(db)
        {
            bail!("failed to compile: {}", path.display());
        }

        Ok(Self {
            db: db.snapshot(),
            test_crate_ids: main_crate_ids.clone(),
            main_crate_ids,
            starknet,
        })
    }

    /// Build the tests and collect metadata.
    pub fn build(&self) -> Result<TestCompilation> {
        compile_test_prepared_db(
            &self.db,
            self.starknet,
            self.main_crate_ids.clone(),
            self.test_crate_ids.clone(),
        )
    }
}

/// Filter compiled test cases with user provided arguments.
///
/// # Arguments
/// * `compiled` - Compiled test cases with metadata.
/// * `include_ignored` - Include ignored tests as well.
/// * `ignored` - Run ignored tests only.l
/// * `filter` - Include only tests containing the filter string.
/// # Returns
/// * (`TestCompilation`, `usize`) - The filtered test cases and the number of filtered out cases.
pub fn filter_test_cases(
    compiled: TestCompilation,
    include_ignored: bool,
    ignored: bool,
    filter: String,
) -> (TestCompilation, usize) {
    let total_tests_count = compiled.named_tests.len();
    let named_tests = compiled
        .named_tests
        .into_iter()
        .map(|(func, mut test)| {
            // Un-ignoring all the tests in `include-ignored` mode.
            if include_ignored {
                test.ignored = false;
            }
            (func, test)
        })
        .filter(|(name, _)| name.contains(&filter))
        // Filtering unignored tests in `ignored` mode
        .filter(|(_, test)| !ignored || test.ignored)
        .collect_vec();
    let filtered_out = total_tests_count - named_tests.len();
    let tests = TestCompilation {
        named_tests,
        ..compiled
    };
    (tests, filtered_out)
}

/// The ran function return value.
#[derive(Debug, Eq, PartialEq, Clone)]
pub enum RunResultValue {
    /// Run ended successfully, returning the memory of the non-implicit returns.
    Success(Vec<VMFelt>),
    /// Run panicked, returning the carried error data.
    Panic(Vec<VMFelt>),
}

/// The status of a ran test.
enum TestStatus {
    Success,
    Fail(RunResultValue),
}

/// The result of a ran test.
struct TestResult {
    /// The status of the run.
    status: TestStatus,
    /// The gas usage of the run if relevant.
    gas_usage: Option<i64>,
}

/// Summary data of the ran tests.
pub struct TestsSummary {
    passed: Vec<String>,
    failed: Vec<String>,
    ignored: Vec<String>,
    failed_run_results: Vec<RunResultValue>,
}

fn is_equal_vec_felt(a: &Vec<VMFelt>, b: &Vec<Felt252>) -> bool {
    a.iter().map(|f| f.to_biguint()).collect_vec()
        != b.iter().map(|f: &Felt252| f.to_biguint()).collect_vec()
}

/// Runs the tests and process the results for a summary.
pub fn run_tests(
    named_tests: Vec<(String, TestConfig)>,
    sierra_program: Program,
    // _function_set_costs: OrderedHashMap<FunctionId, OrderedHashMap<CostTokenType, i32>>,
    // _contracts_info: OrderedHashMap<Felt252, ContractInfo>,
    oracle_server: &Option<String>,
    configuration: &Configuration,
    layout: &LayoutName,
) -> Result<TestsSummary> {
    println!("running {} tests", named_tests.len());
    let wrapped_summary = Mutex::new(Ok(TestsSummary {
        passed: vec![],
        failed: vec![],
        ignored: vec![],
        failed_run_results: vec![],
    }));
    named_tests
        .into_par_iter()
        .map(
            |(name, test)| -> anyhow::Result<(String, Option<TestResult>)> {
                if test.ignored {
                    return Ok((name, None));
                }

                let r = run_1(
                    configuration,
                    oracle_server,
                    layout,
                    &None,
                    &None,
                    &FuncArgs::default(),
                    &sierra_program,
                    &name,
                    false,
                );

                Ok((
                    name,
                    Some(TestResult {
                        status: match r {
                            Ok(_) => match test.expectation {
                                TestExpectation::Success => TestStatus::Success,
                                TestExpectation::Panics(_) => TestStatus::Fail(
                                    RunResultValue::Panic([VMFelt::default()].to_vec()),
                                ),
                            },
                            Err(Error::RunPanic(panic_data)) => match test.expectation {
                                TestExpectation::Success => {
                                    TestStatus::Fail(RunResultValue::Panic(panic_data))
                                }
                                TestExpectation::Panics(panic_expectation) => {
                                    match panic_expectation {
                                        PanicExpectation::Exact(expected)
                                            if !is_equal_vec_felt(&panic_data, &expected) =>
                                        {
                                            TestStatus::Fail(RunResultValue::Panic(panic_data))
                                        }
                                        _ => TestStatus::Success,
                                    }
                                }
                            },
                            Err(_) => panic!("Error!"),
                        },
                        gas_usage: None,
                    }),
                ))
            },
        )
        .for_each(|r| {
            let mut wrapped_summary = wrapped_summary.lock().unwrap();
            if wrapped_summary.is_err() {
                return;
            }
            let (name, status) = match r {
                Ok((name, status)) => (name, status),
                Err(err) => {
                    *wrapped_summary = Err(err);
                    return;
                }
            };
            let summary = wrapped_summary.as_mut().unwrap();
            let (res_type, status_str, gas_usage) = match status {
                Some(TestResult {
                    status: TestStatus::Success,
                    gas_usage,
                }) => (&mut summary.passed, "ok".bright_green(), gas_usage),
                Some(TestResult {
                    status: TestStatus::Fail(run_result),
                    gas_usage,
                }) => {
                    summary.failed_run_results.push(run_result);
                    (&mut summary.failed, "fail".bright_red(), gas_usage)
                }
                None => (&mut summary.ignored, "ignored".bright_yellow(), None),
            };
            if let Some(gas_usage) = gas_usage {
                println!("test {name} ... {status_str} (gas usage est.: {gas_usage})");
            } else {
                println!("test {name} ... {status_str}");
            }
            res_type.push(name);
        });
    wrapped_summary.into_inner().unwrap()
}
