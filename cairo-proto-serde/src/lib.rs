use crate::configuration::{Configuration, FieldType, PrimitiveType};
use num_traits::One;
use num_traits::ToPrimitive;
use num_traits::Zero;
use serde_json::{json, Map, Value};
use starknet_types_core::felt::Felt as Felt252;

pub mod configuration;

fn serialize_primitive(ty: &PrimitiveType, value: &Value) -> Vec<Felt252> {
    let element = match ty {
        PrimitiveType::FELT252 => match value {
            Value::String(s) => Felt252::from_hex(s).expect("Invalid FELT252 hex string"),
            _ => panic!("FELT252 must be a string"),
        },
        PrimitiveType::U64 => Felt252::from(
            value
                .as_u64()
                .expect(format!("Error converting {value:?} to u64").as_str()),
        ),
        PrimitiveType::U32 => Felt252::from(
            value
                .as_u64()
                .expect(format!("Error converting {value:?} to u64").as_str()),
        ),
        PrimitiveType::I32 => Felt252::from(
            value
                .as_i64()
                .expect(format!("Error converting {value:?} to i64").as_str()),
        ),
        PrimitiveType::I64 => Felt252::from(
            value
                .as_i64()
                .expect(format!("Error converting {value:?} to i64").as_str()),
        ),
        PrimitiveType::BYTEARRAY => {
            let mut p = Vec::new();
            let bytes = value.as_str().unwrap().as_bytes();

            let total_length = bytes.len().to_u32().unwrap() / 31;
            p.push(Felt252::from(total_length));

            bytes
                .chunks(31)
                .for_each(|v| p.push(Felt252::from_bytes_be_slice(v)));

            let last_row_length = bytes.len().to_u32().unwrap() % 31;
            if last_row_length == 0 {
                p.push(Felt252::from(0));
            }
            p.push(Felt252::from(last_row_length));
            return p;
        }
        PrimitiveType::BOOL => Felt252::from(
            value
                .as_bool()
                .expect(format!("Error converting {value} to bool").as_str()),
        ),
    };
    vec![element]
}

fn deserialize_primitive(ty: &PrimitiveType, value: &mut &[Felt252]) -> Value {
    let num = value[0].to_bigint();
    *value = &value[1..];

    match ty {
        PrimitiveType::FELT252 => {
            let hex_string = format!("0x{}", num.to_str_radix(16));
            json!(hex_string)
        }
        PrimitiveType::U64 => {
            json!(u64::try_from(num).expect(format!("Error converting {value:?} to u64").as_str()))
        }
        PrimitiveType::U32 => {
            json!(u32::try_from(num).expect(format!("Error converting {value:?} to u32").as_str()))
        }
        PrimitiveType::I32 => {
            json!(i32::try_from(num).expect(format!("Error converting {value:?} to i32").as_str()))
        }
        PrimitiveType::I64 => {
            json!(i64::try_from(num).expect(format!("Error converting {value:?} to i64").as_str()))
        }
        PrimitiveType::BYTEARRAY => {
            let data_len = usize::try_from(num).unwrap();
            let data = &value[0..data_len];
            let pending_word = value[data_len];
            let pending_word_len = value[data_len + 1].to_u32().unwrap() as usize;
            let trim_len = 32 - pending_word_len;
            *value = &value[(data_len + 2)..];

            let mut v = Vec::<u8>::with_capacity(31 * data.len());
            for felt in data {
                v.extend_from_slice(&felt.to_bytes_be()[1..]);
            }
            v.extend_from_slice(&pending_word.to_bytes_be()[trim_len..]);

            json!(String::from_utf8(v).unwrap())
        }
        PrimitiveType::BOOL => {
            if num.is_one() {
                json!(true)
            } else if num.is_zero() {
                json!(false)
            } else {
                panic!("{value:#?} can't be converted to boolean")
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
            let message_config = config.messages.get(message_ty).expect(
                format!("Key `{message_ty}` not found in configuration JSON file").as_str(),
            );
            let value = value
                .as_object()
                .expect(format!("must be an object to serialize as message {message_ty}").as_str());
            for field in message_config {
                if let Some(field_value) = value.get(&field.name) {
                    result.append(&mut serialize_cairo_serde(config, &field.ty, field_value));
                } else if let Some(field_value) =
                    value.get(field.name.trim_start_matches("felt252_"))
                {
                    // Try without the "felt252_" prefix
                    result.append(&mut serialize_cairo_serde(config, &field.ty, field_value));
                } else {
                    panic!("Field {} not found in value object", field.name);
                }
            }
        }
        FieldType::Enum(_) => result.append(&mut serialize_primitive(&PrimitiveType::I32, value)),
        FieldType::Option(inner_ty) => {
            if value.is_null() {
                result.append(&mut serialize_primitive(&PrimitiveType::U64, &json!(1)));
            } else {
                result.append(&mut serialize_primitive(&PrimitiveType::U64, &json!(0)));
                result.append(&mut serialize_cairo_serde(config, inner_ty, value));
            }
        }
        FieldType::Array(value_ty) => {
            let value = value.as_array().expect("must be an array");
            result.append(&mut serialize_primitive(
                &PrimitiveType::U64,
                &json!(value.len()),
            ));
            for element in value {
                if let FieldType::Primitive(PrimitiveType::FELT252) = **value_ty {
                    result.append(&mut serialize_primitive(&PrimitiveType::FELT252, element));
                } else {
                    result.append(&mut serialize_cairo_serde(config, value_ty, element));
                }
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
            let message_config = config.messages.get(message_ty).expect(
                format!("Key `{message_ty}` not found in configuration JSON file").as_str(),
            );
            let mut result = Map::new();
            for field in message_config {
                result.insert(
                    field.name.clone(),
                    deserialize_cairo_serde(config, &field.ty, value),
                );
            }
            Value::Object(result)
        }
        FieldType::Enum(_) => deserialize_primitive(&PrimitiveType::I32, value),
        FieldType::Option(inner_ty) => {
            let idx = deserialize_primitive(&PrimitiveType::U64, value);
            if idx == 0 {
                deserialize_cairo_serde(config, inner_ty, value)
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
            for _ in 0..len {
                if let FieldType::Primitive(PrimitiveType::FELT252) = **value_ty {
                    result.push(deserialize_primitive(&PrimitiveType::FELT252, value));
                } else {
                    result.push(deserialize_cairo_serde(config, value_ty, value));
                }
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
    use serde_json::{json, Value};
    use starknet_types_core::felt::Felt as Felt252;
    use std::collections::{BTreeMap, HashMap};

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
        // let new_configuration = serde_json::from_str::<Configuration>(&json_string).unwrap();
        println!("JSON {json_string:?} -> {configuration:?}");
    }

    #[test]
    fn it_handles_multiple_strings() {
        let configuration = test_configuration();
        let json = json!({
            "a": "lorem",
            "b": "this is a somewhat longer string that overflows a single felt",
            "c": "dolor"
        });
        let ty = FieldType::Message("Strings".into());
        let cairo_message = serialize_cairo_serde(&configuration, &ty, &json);
        let deserialized_message =
            deserialize_cairo_serde(&configuration, &ty, &mut cairo_message.as_ref());
        assert_eq!(json, deserialized_message);
    }

    fn test_configuration() -> Configuration {
        let mut messages = BTreeMap::new();
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
            String::from("Strings"),
            vec![
                Field {
                    name: "a".into(),
                    ty: FieldType::Primitive(PrimitiveType::BYTEARRAY),
                },
                Field {
                    name: "b".into(),
                    ty: FieldType::Primitive(PrimitiveType::BYTEARRAY),
                },
                Field {
                    name: "c".into(),
                    ty: FieldType::Primitive(PrimitiveType::BYTEARRAY),
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

        let mut services = BTreeMap::new();
        services.insert(String::from("SqrtOracle"), Service { methods });

        let enums = BTreeMap::new();
        Configuration {
            enums,
            messages,
            services,
        }
    }
}
