use cairo_felt::Felt252;
use serde_json::{json, Map, Value};
use crate::configuration::{Configuration, FieldType};

pub mod configuration;

// TODO: cover all types
fn serialize_primitive(ty: &str, value: &Value) -> Vec<Felt252> {
    let x = value.as_number().unwrap().as_u64().unwrap();
    vec![Felt252::from(x)]
}

fn deserialize_primitive(ty: &str, value: &mut &[Felt252]) -> Value {
    let x: u64 = value[0].to_bigint().try_into().unwrap();
    *value = &value[1..];
    json!(x)
}

fn serialize_cairo_serde(config: &Configuration, ty: &FieldType, value: &Value) -> Vec<Felt252> {
    let mut result = Vec::new();

    match ty {
        FieldType::Primitive(ty) => result.append(&mut serialize_primitive(ty, value)),
        FieldType::Message(message_ty) => {
            let message_config = &config.messages[message_ty];
            let value = value.as_object().expect("must be an object to serialize as message {message_ty}");
            for field in message_config {
                result.append(&mut serialize_cairo_serde(config, &field.ty, &value[&field.name]));
            }
        }
        FieldType::Option(inner_ty) => {
            if value.is_null() {
                result.append(&mut serialize_primitive("u64".into(), &json!(1)));
            } else {
                result.append(&mut serialize_primitive("u64".into(), &json!(0)));
                result.append(&mut serialize_cairo_serde(config, &inner_ty, value));
            }
        }
        FieldType::Array(value_ty) => {
            let value = value.as_array().expect("must be an array");
            result.append(&mut serialize_primitive("u64".into(), &json!(value.len())));
            for element in value {
                result.append(&mut serialize_cairo_serde(config, &value_ty, element));
            }
        }
    }

    result
}

fn deserialize_cairo_serde(config: &Configuration, ty: &FieldType, value: &mut &[Felt252]) -> Value {
    match ty {
        FieldType::Primitive(ty) => deserialize_primitive(ty, value),
        FieldType::Message(message_ty) => {
            let message_config = &config.messages[message_ty];
            let mut result = Map::new();
            for field in message_config {
                result.insert(field.name.clone(), deserialize_cairo_serde(config, &field.ty, value));
            }
            Value::Object(result)
        }
        FieldType::Option(inner_ty) => {
            let idx = deserialize_primitive("u64", value);
            if idx == 0 {
                deserialize_cairo_serde(config, &inner_ty, value)
            } else {
                Value::Null
            }
        }
        FieldType::Array(value_ty) => {
            let len = deserialize_primitive("u64", value).as_number().unwrap().as_u64().unwrap() as usize;
            let mut result = Vec::new();
            for _i in 0..len {
                result.push(deserialize_cairo_serde(config, &value_ty, value));
            }
            Value::Array(result)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use cairo_felt::Felt252;
    use serde_json::{json, Value};
    use crate::configuration::{Configuration, Field, FieldType, MethodDeclaration, Service};
    use crate::{deserialize_cairo_serde, serialize_cairo_serde};

    #[test]
    fn it_serializes_cairo_serde() {
        let json = json!({ "n": 42 });
        let message_type = "Response";
        let configuration = test_configuration();
        let cairo_message = serialize_cairo_serde(&configuration, &FieldType::Message(message_type.into()), &json);

        println!("configuration {configuration:?}");
        println!("result {cairo_message:?}");
    }

    #[test]
    fn it_deserializes_cairo_serde() {
        let cairo_message = vec![Felt252::from(42 * 42), Felt252::from(1), Felt252::from(1), Felt252::from(18)];
        let message_type = "Request";
        let configuration = test_configuration();
        let deserialized = deserialize_cairo_serde(&configuration, &FieldType::Message(message_type.into()), &mut cairo_message.as_ref());
        let expected_json = json!({
            "n": 42 * 42,
            "x": Value::Null,
            "y": vec![18]
        });

        assert_eq!(deserialized, expected_json);
    }


    fn test_configuration() -> Configuration {
        let mut messages = HashMap::new();
        messages.insert(
            String::from("Inner"), vec![
                Field { name: "inner".into(), ty: FieldType::Primitive("u32".into()) },
            ]);
        messages.insert(
            String::from("Request"), vec![
                Field { name: "n".into(), ty: FieldType::Primitive("u64".into()) },
                Field { name: "x".into(), ty: FieldType::Option(Box::new(FieldType::Message("Inner".into()))) },
                Field { name: "y".into(), ty: FieldType::Array(Box::new(FieldType::Primitive("i64".into()))) },
            ]);
        messages.insert(
            String::from("Response"), vec![
                Field { name: "n".into(), ty: FieldType::Primitive("u64".into()) },
            ]);

        let mut methods = HashMap::new();
        methods.insert(String::from("sqrt"), MethodDeclaration {
            input: FieldType::Message("Request".into()),
            output: FieldType::Message("Response".into()),
        });

        let mut services = HashMap::new();
        services.insert(
            String::from("SqrtOracle"), Service { methods }
        );

        Configuration {
            messages,
            services,
        }
    }
}
