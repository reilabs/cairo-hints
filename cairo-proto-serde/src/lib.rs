use crate::configuration::{Configuration, FieldType, PrimitiveType};
use cairo_felt::Felt252;
use num_traits::identities::One;
use num_traits::identities::Zero;
use serde_json::{json, Map, Value};

pub mod configuration;

fn serialize_primitive(ty: &PrimitiveType, value: &Value) -> Vec<Felt252> {
    let element = match ty {
        PrimitiveType::U64 => Felt252::from(value.as_u64().unwrap()),
        PrimitiveType::U32 => Felt252::from(value.as_u64().unwrap()),
        PrimitiveType::I32 => Felt252::from(value.as_i64().unwrap()),
        PrimitiveType::I64 => Felt252::from(value.as_i64().unwrap()),
        PrimitiveType::BOOL => Felt252::from(value.as_bool().unwrap()),
    };
    vec![element]
}

fn deserialize_primitive(ty: &PrimitiveType, value: &mut &[Felt252]) -> Value {
    let num = value[0].to_bigint();
    *value = &value[1..];

    match ty {
        PrimitiveType::U64 => json!(u64::try_from(num).unwrap()),
        PrimitiveType::U32 => json!(u32::try_from(num).unwrap()),
        PrimitiveType::I32 => json!(i32::try_from(num).unwrap()),
        PrimitiveType::I64 => json!(i64::try_from(num).unwrap()),
        PrimitiveType::BOOL => {
            if num.is_one() {
                json!(true)
            } else if num.is_zero() {
                json!(false)
            } else {
                panic!("Value can't be converted to boolean")
            }
        }
    }
}

pub fn serialize_cairo_serde(
    config: &Configuration,
    ty: &FieldType,
    value: &Value,
) -> Vec<Felt252> {
    let mut result = Vec::new();

    match ty {
        FieldType::Primitive(ty) => result.append(&mut serialize_primitive(ty, value)),
        FieldType::Message(message_ty) => {
            let message_config = &config.messages[message_ty];
            let value = value
                .as_object()
                .expect("must be an object to serialize as message {message_ty}");
            for field in message_config {
                result.append(&mut serialize_cairo_serde(
                    config,
                    &field.ty,
                    &value[&field.name],
                ));
            }
        }
        FieldType::Option(inner_ty) => {
            if value.is_null() {
                result.append(&mut serialize_primitive(&PrimitiveType::U64, &json!(1)));
            } else {
                result.append(&mut serialize_primitive(&PrimitiveType::U64, &json!(0)));
                result.append(&mut serialize_cairo_serde(config, &inner_ty, value));
            }
        }
        FieldType::Array(value_ty) => {
            let value = value.as_array().expect("must be an array");
            result.append(&mut serialize_primitive(
                &PrimitiveType::U64,
                &json!(value.len()),
            ));
            for element in value {
                result.append(&mut serialize_cairo_serde(config, &value_ty, element));
            }
        }
    }

    result
}

pub fn deserialize_cairo_serde(
    config: &Configuration,
    ty: &FieldType,
    value: &mut &[Felt252],
) -> Value {
    match ty {
        FieldType::Primitive(ty) => deserialize_primitive(ty, value),
        FieldType::Message(message_ty) => {
            let message_config = &config.messages[message_ty];
            let mut result = Map::new();
            for field in message_config {
                result.insert(
                    field.name.clone(),
                    deserialize_cairo_serde(config, &field.ty, value),
                );
            }
            Value::Object(result)
        }
        FieldType::Option(inner_ty) => {
            let idx = deserialize_primitive(&PrimitiveType::U64, value);
            if idx == 0 {
                deserialize_cairo_serde(config, &inner_ty, value)
            } else {
                Value::Null
            }
        }
        FieldType::Array(value_ty) => {
            let len = deserialize_primitive(&PrimitiveType::U64, value)
                .as_number()
                .unwrap()
                .as_u64()
                .unwrap() as usize;
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
    use crate::configuration::{
        Configuration, Field, FieldType, MethodDeclaration, PrimitiveType, Service,
    };
    use crate::{deserialize_cairo_serde, serialize_cairo_serde};
    use cairo_felt::Felt252;
    use serde_json::{json, Value};
    use std::collections::HashMap;

    #[test]
    fn it_serializes_cairo_serde() {
        let json = json!({ "n": 42 });
        let message_type = "Response";
        let configuration = test_configuration();
        let cairo_message = serialize_cairo_serde(
            &configuration,
            &FieldType::Message(message_type.into()),
            &json,
        );

        println!("configuration {configuration:?}");
        println!("result {cairo_message:?}");
    }

    #[test]
    fn it_deserializes_cairo_serde() {
        let cairo_message = vec![
            Felt252::from(42 * 42),
            Felt252::from(1),
            Felt252::from(1),
            Felt252::from(18),
        ];
        let message_type = "Request";
        let configuration = test_configuration();
        let deserialized = deserialize_cairo_serde(
            &configuration,
            &FieldType::Message(message_type.into()),
            &mut cairo_message.as_ref(),
        );
        let expected_json = json!({
            "n": 42 * 42,
            "x": Value::Null,
            "y": vec![18]
        });

        assert_eq!(deserialized, expected_json);
    }

    #[test]
    fn it_saves_configuration() {
        let configuration = test_configuration();
        let json_string = serde_json::to_string(&configuration).unwrap();
        let new_configuration = serde_json::from_str::<Configuration>(&json_string).unwrap();
        println!("JSON {json_string:?} -> {configuration:?}");
    }

    fn test_configuration() -> Configuration {
        let mut messages = HashMap::new();
        messages.insert(
            String::from("Inner"),
            vec![Field {
                name: "inner".into(),
                ty: FieldType::Primitive(PrimitiveType::U32),
            }],
        );
        messages.insert(
            String::from("Request"),
            vec![
                Field {
                    name: "n".into(),
                    ty: FieldType::Primitive(PrimitiveType::U64),
                },
                Field {
                    name: "x".into(),
                    ty: FieldType::Option(Box::new(FieldType::Message("Inner".into()))),
                },
                Field {
                    name: "y".into(),
                    ty: FieldType::Array(Box::new(FieldType::Primitive(PrimitiveType::I32))),
                },
            ],
        );
        messages.insert(
            String::from("Response"),
            vec![Field {
                name: "n".into(),
                ty: FieldType::Primitive(PrimitiveType::U64),
            }],
        );

        let mut methods = HashMap::new();
        methods.insert(
            String::from("sqrt"),
            MethodDeclaration {
                input: FieldType::Message("Request".into()),
                output: FieldType::Message("Response".into()),
            },
        );

        let mut services = HashMap::new();
        services.insert(String::from("SqrtOracle"), Service { methods });

        Configuration { messages, services }
    }
}
