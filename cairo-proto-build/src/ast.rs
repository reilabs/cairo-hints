use prost_types::source_code_info::Location;

/// Comments on a Protobuf item.
#[derive(Debug, Default, Clone)]
pub struct Comments {
    /// Leading detached blocks of comments.
    pub leading_detached: Vec<Vec<String>>,

    /// Leading comments.
    pub leading: Vec<String>,

    /// Trailing comments.
    pub trailing: Vec<String>,
}

impl Comments {
    pub(crate) fn from_location(location: &Location) -> Comments {
        let leading_detached = location
            .leading_detached_comments
            .iter()
            .map(get_lines)
            .collect();
        let leading = location
            .leading_comments
            .as_ref()
            .map_or(Vec::new(), get_lines);
        let trailing = location
            .trailing_comments
            .as_ref()
            .map_or(Vec::new(), get_lines);
        Comments {
            leading_detached,
            leading,
            trailing,
        }
    }
}

/// A service descriptor.
#[derive(Debug, Clone)]
pub struct Service {
    /// The service name in Rust style.
    pub name: String,
    /// The service name as it appears in the .proto file.
    pub proto_name: String,
    /// The package name as it appears in the .proto file.
    pub package: String,
    /// The service comments.
    pub comments: Comments,
    /// The service methods.
    pub methods: Vec<Method>,
    /// The service options.
    pub options: prost_types::ServiceOptions,
}

/// A service method descriptor.
#[derive(Debug, Clone)]
pub struct Method {
    /// The name of the method in Rust style.
    pub name: String,
    /// The name of the method as it appears in the .proto file.
    pub proto_name: String,
    /// The method comments.
    pub comments: Comments,
    /// The input Rust type.
    pub input_type: String,
    /// The output Rust type.
    pub output_type: String,
    /// The input Protobuf type.
    pub input_proto_type: String,
    /// The output Protobuf type.
    pub output_proto_type: String,
    /// The method options.
    pub options: prost_types::MethodOptions,
    /// Identifies if client streams multiple client messages.
    pub client_streaming: bool,
    /// Identifies if server streams multiple server messages.
    pub server_streaming: bool,
}

#[cfg(not(feature = "cleanup-markdown"))]
fn get_lines<S>(comments: S) -> Vec<String>
where
    S: AsRef<str>,
{
    comments.as_ref().lines().map(str::to_owned).collect()
}

#[cfg(feature = "cleanup-markdown")]
fn get_lines<S>(comments: S) -> Vec<String>
where
    S: AsRef<str>,
{
    let comments = comments.as_ref();
    let mut buffer = String::with_capacity(comments.len() + 256);
    let opts = pulldown_cmark_to_cmark::Options {
        code_block_token_count: 3,
        ..Default::default()
    };
    match pulldown_cmark_to_cmark::cmark_with_options(
        Parser::new_ext(comments, Options::all() - Options::ENABLE_SMART_PUNCTUATION).map(
            |event| {
                fn map_codeblock(kind: CodeBlockKind) -> CodeBlockKind {
                    match kind {
                        CodeBlockKind::Fenced(s) => {
                            if &*s == "rust" {
                                CodeBlockKind::Fenced("compile_fail".into())
                            } else {
                                CodeBlockKind::Fenced(format!("text,{}", s).into())
                            }
                        }
                        CodeBlockKind::Indented => CodeBlockKind::Fenced("text".into()),
                    }
                }
                match event {
                    Event::Start(Tag::CodeBlock(kind)) => {
                        Event::Start(Tag::CodeBlock(map_codeblock(kind)))
                    }
                    Event::End(Tag::CodeBlock(kind)) => {
                        Event::End(Tag::CodeBlock(map_codeblock(kind)))
                    }
                    e => e,
                }
            },
        ),
        &mut buffer,
        opts,
    ) {
        Ok(_) => buffer.lines().map(str::to_owned).collect(),
        Err(_) => comments.lines().map(str::to_owned).collect(),
    }
}
