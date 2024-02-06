use cairo_proto_serde::configuration::Configuration;
use cairo_proto_serde::configuration::Field;
use cairo_proto_serde::configuration::FieldType;
use cairo_proto_serde::configuration::Mapping;
use cairo_proto_serde::configuration::Service;
use code_generator::CodeGenerator;
use core::fmt::Debug;
use extern_paths::ExternPaths;
use ident::to_snake;
use log::debug;
use log::trace;
use message_graph::MessageGraph;
use path::PathMap;
use prost::Message;
use prost_types::FileDescriptorProto;
use prost_types::FileDescriptorSet;
use std::collections::HashMap;
use std::default;
use std::env;
use std::fmt;
use std::fs;
use std::io::{Error, ErrorKind};
use std::ops::RangeBounds;
use std::ops::RangeToInclusive;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str::FromStr;
use std::sync::Once;

mod ast;
mod code_generator;
mod extern_paths;
mod ident;
mod message_graph;
mod path;

/// The map collection type to output for Protobuf `map` fields.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq)]
enum MapType {
    /// The [`std::collections::HashMap`] type.
    HashMap,
    /// The [`std::collections::BTreeMap`] type.
    BTreeMap,
}

impl Default for MapType {
    fn default() -> MapType {
        MapType::HashMap
    }
}

/// The bytes collection type to output for Protobuf `bytes` fields.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq)]
enum BytesType {
    /// The [`alloc::collections::Vec::<u8>`] type.
    Vec,
    /// The [`bytes::Bytes`] type.
    Bytes,
}

impl Default for BytesType {
    fn default() -> BytesType {
        BytesType::Vec
    }
}

/// A Rust module path for a Protobuf package.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Module {
    components: Vec<String>,
}

impl Module {
    /// Construct a module path from an iterator of parts.
    pub fn from_parts<I>(parts: I) -> Self
    where
        I: IntoIterator,
        I::Item: Into<String>,
    {
        Self {
            components: parts.into_iter().map(|s| s.into()).collect(),
        }
    }

    /// Construct a module path from a Protobuf package name.
    ///
    /// Constituent parts are automatically converted to snake case in order to follow
    /// Rust module naming conventions.
    pub fn from_protobuf_package_name(name: &str) -> Self {
        Self {
            components: name
                .split('.')
                .filter(|s| !s.is_empty())
                .map(to_snake)
                .collect(),
        }
    }

    /// An iterator over the parts of the path.
    pub fn parts(&self) -> impl Iterator<Item = &str> {
        self.components.iter().map(|s| s.as_str())
    }

    /// Format the module path into a filename for generated Rust code.
    ///
    /// If the module path is empty, `default` is used to provide the root of the filename.
    pub fn to_file_name_or(&self, default: &str) -> String {
        let mut root = if self.components.is_empty() {
            default.to_owned()
        } else {
            self.components.join(".")
        };

        root.push_str(".cairo");

        root
    }

    /// The number of parts in the module's path.
    pub fn len(&self) -> usize {
        self.components.len()
    }

    /// Whether the module's path contains any components.
    pub fn is_empty(&self) -> bool {
        self.components.is_empty()
    }

    fn to_partial_file_name(&self, range: RangeToInclusive<usize>) -> String {
        self.components[range].join(".")
    }

    fn part(&self, idx: usize) -> &str {
        self.components[idx].as_str()
    }
}

impl fmt::Display for Module {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut parts = self.parts();
        if let Some(first) = parts.next() {
            f.write_str(first)?;
        }
        for part in parts {
            f.write_str("::")?;
            f.write_str(part)?;
        }
        Ok(())
    }
}

pub struct Config {
    boxed: PathMap<()>,
    out_dir: Option<PathBuf>,
    default_package_filename: String,
}

impl Config {
    /// Creates a new code generator configuration with default options.
    pub fn new() -> Config {
        Config::default()
    }

    /// Configures the output directory where generated Rust files will be written.
    ///
    /// If unset, defaults to the `OUT_DIR` environment variable. `OUT_DIR` is set by Cargo when
    /// executing build scripts, so `out_dir` typically does not need to be configured.
    pub fn out_dir<P>(&mut self, path: P) -> &mut Self
    where
        P: Into<PathBuf>,
    {
        self.out_dir = Some(path.into());
        self
    }

    /// Compile `.proto` files into Rust files during a Cargo build with additional code generator
    /// configuration options.
    ///
    /// This method is like the `prost_build::compile_protos` function, with the added ability to
    /// specify non-default code generation options. See that function for more information about
    /// the arguments and generated outputs.
    pub fn compile_protos(
        &mut self,
        protos: &[impl AsRef<Path>],
        includes: &[impl AsRef<Path>],
    ) -> std::io::Result<()> {
        // TODO: This should probably emit 'rerun-if-changed=PATH' directives for cargo, however
        // according to [1] if any are output then those paths replace the default crate root,
        // which is undesirable. Figure out how to do it in an additive way; perhaps gcc-rs has
        // this figured out.
        // [1]: http://doc.crates.io/build-script.html#outputs-of-the-build-script

        let tmp = tempfile::Builder::new().prefix("prost-build").tempdir()?;
        let file_descriptor_set_path = tmp.path().join("prost-descriptor-set");

        let protoc = protoc_from_env();

        let mut cmd = Command::new(protoc.clone());
        cmd.arg("--include_imports")
            .arg("--include_source_info")
            .arg("-o")
            .arg(&file_descriptor_set_path);

        for include in includes {
            if include.as_ref().exists() {
                cmd.arg("-I").arg(include.as_ref());
            } else {
                debug!(
                    "ignoring {} since it does not exist.",
                    include.as_ref().display()
                )
            }
        }

        // Set the protoc include after the user includes in case the user wants to
        // override one of the built-in .protos.
        if let Some(protoc_include) = protoc_include_from_env() {
            cmd.arg("-I").arg(protoc_include);
        }

        for proto in protos {
            cmd.arg(proto.as_ref());
        }

        debug!("Running: {:?}", cmd);

        println!("compile_protos {:#?}", cmd);
        let output = cmd.output().map_err(|error| {
            Error::new(
                error.kind(),
                format!("failed to invoke protoc (hint: https://docs.rs/prost-build/#sourcing-protoc): (path: {:?}): {}", &protoc, error),
            )
        })?;

        if !output.status.success() {
            return Err(Error::new(
                ErrorKind::Other,
                format!("protoc failed: {}", String::from_utf8_lossy(&output.stderr)),
            ));
        }

        let buf = fs::read(&file_descriptor_set_path).map_err(|e| {
            Error::new(
                e.kind(),
                format!(
                    "unable to open file_descriptor_set_path: {:?}, OS: {}",
                    &file_descriptor_set_path, e
                ),
            )
        })?;
        let file_descriptor_set = FileDescriptorSet::decode(&*buf).map_err(|error| {
            Error::new(
                ErrorKind::InvalidInput,
                format!("invalid FileDescriptorSet: {}", error),
            )
        })?;

        self.compile_fds(protos, file_descriptor_set)
    }

    /// Compile a [`FileDescriptorSet`] into Rust files during a Cargo build with
    /// additional code generator configuration options.
    ///
    /// This method is like `compile_protos` function except it does not invoke `protoc`
    /// and instead requires the user to supply a [`FileDescriptorSet`].
    ///
    /// # Example `build.rs`
    ///
    /// ```rust,no_run
    /// # use cairo_proto_build::Config;
    /// # use prost_types::FileDescriptorSet;
    /// # fn fds() -> FileDescriptorSet { todo!() }
    /// fn main() -> std::io::Result<()> {
    ///   let file_descriptor_set = fds();
    ///
    ///   Config::new()
    ///     .compile_fds(file_descriptor_set)
    /// }
    /// ```
    pub fn compile_fds(
        &mut self,
        protos: &[impl AsRef<Path>],
        fds: FileDescriptorSet,
    ) -> std::io::Result<()> {
        let target: PathBuf = self.out_dir.clone().ok_or_else(|| {
            Error::new(ErrorKind::Other, "out_dir configuration option is not set")
        })?;

        let requests = fds
            .file
            .into_iter()
            .map(|descriptor| {
                (
                    Module::from_protobuf_package_name(descriptor.package()),
                    descriptor,
                )
            })
            .collect::<Vec<_>>();

        let file_names = requests
            .iter()
            .map(|req| {
                (
                    req.0.clone(),
                    req.0.to_file_name_or(&self.default_package_filename),
                )
            })
            .collect::<HashMap<Module, String>>();

        let modules = self.generate(protos, requests)?;

        for (module, content) in &modules {
            let file_name = file_names
                .get(module)
                .expect("every module should have a filename");
            // assuming one component == one package per module
            let component = &module.components[0];
            let list_paths: Vec<String> = protos
                .iter()
                .map(|p| p.as_ref().to_str().unwrap().to_string())
                .collect();

            // Extract only the json matching the protos
            let code_output_path = target.join(file_name);

            let unchanged_code = fs::read(&code_output_path)
                .map(|previous_content| previous_content == content.0.as_bytes())
                .unwrap_or(false);

            if unchanged_code {
                trace!("unchanged code: {:?}", file_name);
            } else {
                trace!("writing code: {:?}", file_name);
                fs::write(code_output_path, &content.0)?;
            }

            // Writing the JSON only for files belonging to `protos`
            if list_paths.iter().any(|p| p.contains(component)) {
                let config_output_path = target.join(&format!("{file_name}.json"));
                let config_json = serde_json::to_string(&content.1).unwrap();
                let unchanged_config = fs::read(&config_output_path)
                    .map(|previous_content| previous_content == config_json.as_bytes())
                    .unwrap_or(false);

                if unchanged_config {
                    trace!("unchanged config: {:?}", file_name);
                } else {
                    trace!("writing config: {:?}", file_name);
                    fs::write(config_output_path, &config_json)?;
                }
            }
        }

        Ok(())
    }

    /// Processes a set of modules and file descriptors, returning a map of modules to generated
    /// code contents.
    ///
    /// This is generally used when control over the output should not be managed by Prost,
    /// such as in a flow for a `protoc` code generating plugin. When compiling as part of a
    /// `build.rs` file, instead use [`compile_protos()`].
    pub fn generate(
        &mut self,
        protos: &[impl AsRef<Path>],
        requests: Vec<(Module, FileDescriptorProto)>,
    ) -> std::io::Result<HashMap<Module, (String, Configuration)>> {
        let mut modules = HashMap::new();
        let mut packages = HashMap::new();

        let message_graph = MessageGraph::new(requests.iter().map(|x| &x.1))
            .map_err(|error| Error::new(ErrorKind::InvalidInput, error))?;
        let extern_paths = ExternPaths::new(&[], true)
            .map_err(|error| Error::new(ErrorKind::InvalidInput, error))?;

        // println!("generate {:#?}", requests);

        for (request_module, request_fd) in requests {
            // Only record packages that have services
            if !request_fd.service.is_empty() {
                packages.insert(request_module.clone(), request_fd.package().to_string());
            }
            let (code_buf, config_buf) =
                modules.entry(request_module.clone()).or_insert_with(|| {
                    let mut init_buf = String::new();
                    Config::append_header(&mut init_buf);
                    (init_buf, Configuration::default())
                });
            CodeGenerator::generate(
                self,
                &message_graph,
                &extern_paths,
                request_fd,
                code_buf,
                config_buf,
            );
            if code_buf.is_empty() {
                // Did not generate any code, remove from list to avoid inclusion in include file or output file list
                modules.remove(&request_module);
            }
        }

        for p in protos {
            let path = p.as_ref().to_str().unwrap().to_string();
            let mut super_enums: HashMap<String, Vec<Mapping>> = HashMap::new();
            let mut super_messages: HashMap<String, Vec<Field>> = HashMap::new();
            let mut super_services: HashMap<String, Service> = HashMap::new();

            // assuming one component == one package per module
            for (module, content) in &modules {
                if path.contains(&module.components[0]) {
                    continue;
                }

                for (name, v) in &content.1.enums {
                    let k = format!("super::{}::{}", module.components[0], name);
                    super_enums.insert(k, v.to_owned());
                }

                for (name, v) in &content.1.messages {
                    let k = format!("super::{}::{}", module.components[0], name);
                    super_messages.insert(k, v.to_owned());
                }

                for (name, v) in &content.1.services {
                    let k = format!("super::{}::{}", module.components[0], name);
                    super_services.insert(k, v.to_owned());
                }
            }

            for (module, content) in &mut modules {
                if !path.contains(&module.components[0]) {
                    continue;
                }

                for (k, v) in &super_enums {
                    content.1.enums.insert(k.to_owned(), v.to_owned());
                }

                for (k, v) in &super_messages {
                    content.1.messages.insert(k.to_owned(), v.to_owned());
                }

                for (k, v) in &super_services {
                    content.1.services.insert(k.to_owned(), v.to_owned());
                }
            }
        }

        self.fmt_modules(&mut modules);

        Ok(modules)
    }

    // Not used yet. If used, it requires checking the last iteration in the for loop.
    // fn append_footer(&mut self, code_buf: &mut String) {}

    fn append_header(code_buf: &mut String) {
        code_buf.push_str("use starknet::testing::cheatcode;\n");
    }

    #[cfg(feature = "format")]
    fn fmt_modules(&mut self, modules: &mut HashMap<Module, (String, Configuration)>) {
        for buf in modules.values_mut() {
            let file = syn::parse_file(buf).unwrap();
            let formatted = prettyplease::unparse(&file);
            *buf = formatted;
        }
    }

    #[cfg(not(feature = "format"))]
    fn fmt_modules(&mut self, _: &mut HashMap<Module, (String, Configuration)>) {}
}

impl default::Default for Config {
    fn default() -> Config {
        Config {
            boxed: PathMap::default(),
            out_dir: None,
            default_package_filename: String::from("oracle"),
        }
    }
}

/// Returns the path to the `protoc` binary.
pub fn protoc_from_env() -> PathBuf {
    let os_specific_hint = if cfg!(target_os = "macos") {
        "You could try running `brew install protobuf` or downloading it from https://github.com/protocolbuffers/protobuf/releases"
    } else if cfg!(target_os = "linux") {
        "If you're on debian, try `apt-get install protobuf-compiler` or download it from https://github.com/protocolbuffers/protobuf/releases"
    } else {
        "You can download it from https://github.com/protocolbuffers/protobuf/releases or from your package manager."
    };
    let error_msg =
        "Could not find `protoc` installation and this build crate cannot proceed without
    this knowledge. If `protoc` is installed and this crate had trouble finding
    it, you can set the `PROTOC` environment variable with the specific path to your
    installed `protoc` binary.";
    let msg = format!(
        "{}{}

For more information: https://docs.rs/prost-build/#sourcing-protoc
",
        error_msg, os_specific_hint
    );

    env::var_os("PROTOC")
        .map(PathBuf::from)
        .or_else(|| which::which("protoc").ok())
        .expect(&msg)
}

/// Returns the path to the Protobuf include directory.
pub fn protoc_include_from_env() -> Option<PathBuf> {
    let protoc_include: PathBuf = env::var_os("PROTOC_INCLUDE")?.into();

    if !protoc_include.exists() {
        panic!(
            "PROTOC_INCLUDE environment variable points to non-existent directory ({:?})",
            protoc_include
        );
    }
    if !protoc_include.is_dir() {
        panic!(
            "PROTOC_INCLUDE environment variable points to a non-directory file ({:?})",
            protoc_include
        );
    }

    Some(protoc_include)
}
