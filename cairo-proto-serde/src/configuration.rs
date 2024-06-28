use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Configuration {
    pub enums: BTreeMap<String, Vec<Mapping>>,
    pub messages: BTreeMap<String, Vec<Field>>,
    pub services: BTreeMap<String, Service>,
    pub servers_config: HashMap<String, String>,
}

// primitive types supported by both Protocol Buffers and Cairo
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum PrimitiveType {
    U64,
    U32,
    I32,
    I64,
    BOOL,
    BYTEARRAY,
    FELT252,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum FieldType {
    Primitive(PrimitiveType),
    Message(String),
    Enum(String),
    Option(Box<FieldType>),
    Array(Box<FieldType>),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Field {
    pub name: String,
    pub ty: FieldType,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Mapping {
    pub name: String,
    pub nb: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(transparent)]
pub struct Service {
    pub methods: HashMap<String, MethodDeclaration>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
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
            "felt252" => FieldType::Primitive(PrimitiveType::FELT252),
            _ => FieldType::Message(value),
        }
    }
}
