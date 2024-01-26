use std::collections::HashMap;
use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Configuration {
    pub messages: HashMap<String, Vec<Field>>,
    pub services: HashMap<String, Service>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum FieldType {
    Primitive(String),
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
