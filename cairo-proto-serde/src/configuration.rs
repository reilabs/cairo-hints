use std::collections::HashMap;
use serde::{Serialize, Deserialize};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Configuration {
    pub messages: HashMap<String, Vec<Field>>,
    pub services: HashMap<String, Service>,
}

// primitive types supported by both Protocol Buffers and Cairo
// TODO: currently it only covers types in the example project
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PrimitiveType {
    U64,
    U32,
    I32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FieldType {
    Primitive(PrimitiveType),
    Message(String),
    Option(Box<FieldType>),
    Array(Box<FieldType>),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Field {
    pub name: String,
    pub ty: FieldType,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Service {
    pub methods: HashMap<String, MethodDeclaration>
}

#[derive(Debug, Serialize, Deserialize)]
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
            _ => FieldType::Message(value)
        }
    }
}