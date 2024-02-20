use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

/// Configuration contains the structure of the JSON file
/// generated from parsing of the protobuf file.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Configuration {
    pub enums: BTreeMap<String, Vec<Mapping>>,
    pub messages: BTreeMap<String, Vec<Field>>,
    pub services: BTreeMap<String, Service>,
}

/// primitive types supported by both Protocol Buffers and Cairo
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PrimitiveType {
    U64,
    U32,
    I32,
    I64,
    BOOL,
    BYTEARRAY,
}

/// List of types allowed in protocol buffuers
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FieldType {
    Primitive(PrimitiveType),
    Message(String),
    Enum(String),
    Option(Box<FieldType>),
    Array(Box<FieldType>),
}

/// Field in protocol buffers
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Field {
    pub name: String,
    pub ty: FieldType,
}

#[doc(hidden)]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Mapping {
    pub name: String,
    pub nb: i32,
}

/// The struct used to start RPC call.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Service {
    pub methods: HashMap<String, MethodDeclaration>,
}

/// Inputs to a `Service`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MethodDeclaration {
    pub input: FieldType,
    pub output: FieldType,
}

impl From<String> for FieldType {
    fn from(value: String) -> Self {
        match value.as_ref() {
            "u64" => FieldType::Primitive(PrimitiveType::U64),
            "u32" => FieldType::Primitive(PrimitiveType::U32),
            "i32" => FieldType::Primitive(PrimitiveType::I32),
            "i64" => FieldType::Primitive(PrimitiveType::I64),
            "bool" => FieldType::Primitive(PrimitiveType::BOOL),
            "ByteArray" => FieldType::Primitive(PrimitiveType::BYTEARRAY),
            _ => FieldType::Message(value),
        }
    }
}
