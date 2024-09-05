use cainome_cairo_serde::ByteArray;
use cairo_oracle_hint_processor::{FuncArg, FuncArgs};
use cairo_vm::Felt252;
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::str::FromStr;

/// Parse a string into `FuncArgs`.
/// Returns an error message if parsing fails or if the format is incorrect.
pub fn process_args(value: &str) -> Result<FuncArgs, String> {
    if value.is_empty() {
        return Ok(FuncArgs::default());
    }
    let mut args = Vec::new();
    let mut input = value.split(' ');
    while let Some(value) = input.next() {
        // First argument in an array
        if value.starts_with('[') {
            if value.ends_with(']') {
                if value.len() == 2 {
                    args.push(FuncArg::Array(Vec::new()));
                } else {
                    args.push(FuncArg::Array(vec![Felt252::from_dec_str(
                        value.strip_prefix('[').unwrap().strip_suffix(']').unwrap(),
                    )
                    .unwrap()]));
                }
            } else {
                let mut array_arg =
                    vec![Felt252::from_dec_str(value.strip_prefix('[').unwrap()).unwrap()];
                // Process following args in array
                let mut array_end = false;
                while !array_end {
                    if let Some(value) = input.next() {
                        // Last arg in array
                        if value.ends_with(']') {
                            array_arg.push(
                                Felt252::from_dec_str(value.strip_suffix(']').unwrap()).unwrap(),
                            );
                            array_end = true;
                        } else {
                            array_arg.push(Felt252::from_dec_str(value).unwrap())
                        }
                    }
                }
                // Finalize array
                args.push(FuncArg::Array(array_arg))
            }
        } else {
            // Single argument
            args.push(FuncArg::Single(Felt252::from_dec_str(value).unwrap()))
        }
    }
    Ok(FuncArgs(args))
}

#[derive(Debug, Clone)]
enum InputType {
    U64,
    I64,
    U32,
    I32,
    U16,
    I16,
    U8,
    I8,
    F64,
    Felt252,
    ByteArray,
    Bool,
    Struct(String),
    Array(Box<InputType>),
    Span(Box<InputType>),
}

#[derive(Debug, Clone)]
struct StructDef {
    fields: Vec<(String, InputType)>,
}

#[derive(Debug)]
pub struct InputSchema {
    structs: BTreeMap<String, StructDef>,
    main_struct: String,
}

pub fn parse_input_schema(file_path: &PathBuf) -> Result<InputSchema, String> {
    let file = File::open(file_path).map_err(|e| format!("Failed to open file: {}", e))?;
    let reader = BufReader::new(file);
    let mut input_schema = InputSchema {
        structs: BTreeMap::new(),
        main_struct: String::new(),
    };
    let mut current_struct: Option<(String, StructDef)> = None;

    for line in reader.lines() {
        let line = line.map_err(|e| format!("Failed to read line: {}", e))?;
        let line = line.trim();

        if line.is_empty() || line.starts_with("//") {
            continue;
        }

        if line.ends_with("{") {
            let struct_name = line.trim_end_matches("{").trim().to_string();
            current_struct = Some((struct_name.clone(), StructDef { fields: Vec::new() }));
            if input_schema.main_struct.is_empty() {
                input_schema.main_struct = struct_name;
            }
        } else if line == "}" {
            if let Some((name, struct_def)) = current_struct.take() {
                input_schema.structs.insert(name, struct_def);
            }
        } else if let Some((_, struct_def)) = &mut current_struct {
            let parts: Vec<&str> = line.split(':').map(|s| s.trim()).collect();
            if parts.len() == 2 {
                let field_name = parts[0].to_string();
                let field_type = parse_type(parts[1])?;
                struct_def.fields.push((field_name, field_type));
            }
        }
    }

    Ok(input_schema)
}

pub fn process_json_args(json_str: &str, schema: &InputSchema) -> Result<FuncArgs, String> {
    let json: Value =
        serde_json::from_str(json_str).map_err(|e| format!("Failed to parse JSON: {}", e))?;
    parse_struct(&json, &schema.main_struct, schema)
}

fn parse_type(type_str: &str) -> Result<InputType, String> {
    match type_str {
        "u64" => Ok(InputType::U64),
        "i64" => Ok(InputType::I64),
        "u32" => Ok(InputType::U32),
        "i32" => Ok(InputType::I32),
        "u16" => Ok(InputType::U16),
        "i16" => Ok(InputType::I16),
        "u8" => Ok(InputType::U8),
        "i8" => Ok(InputType::I8),
        "f64" => Ok(InputType::F64),
        "felt252" => Ok(InputType::Felt252),
        "ByteArray" => Ok(InputType::ByteArray),
        "bool" => Ok(InputType::Bool),
        s if s.starts_with("Array<") => {
            let inner_type = s.trim_start_matches("Array<").trim_end_matches('>');
            Ok(InputType::Array(Box::new(parse_type(inner_type)?)))
        }
        s if s.starts_with("Span<") => {
            let inner_type = s.trim_start_matches("Span<").trim_end_matches('>');
            Ok(InputType::Span(Box::new(parse_type(inner_type)?)))
        }
        s => Ok(InputType::Struct(s.to_string())),
    }
}

fn parse_value(
    value: &Value,
    ty: &InputType,
    schema: &InputSchema,
) -> Result<Vec<FuncArg>, String> {
    match ty {
        InputType::U64 | InputType::U32 | InputType::U16 | InputType::U8 => {
            let num = value
                .as_u64()
                .ok_or_else(|| format!("Expected unsigned integer for {:?}", ty))?;
            Ok(vec![FuncArg::Single(Felt252::from(num))])
        }
        InputType::I64 | InputType::I32 | InputType::I16 | InputType::I8 => {
            let num = value
                .as_i64()
                .ok_or_else(|| format!("Expected signed integer for {:?}", ty))?;
            Ok(vec![FuncArg::Single(Felt252::from(num))])
        }

        InputType::F64 => {
            let num = value
                .as_f64()
                .ok_or_else(|| format!("Expected signed integer for {:?}", ty))?;

            Ok(vec![FuncArg::Single(Felt252::from(
                (num * 2.0_f64.powi(32)) as i64,
            ))])
        }

        InputType::Felt252 => {
            let string = value
                .as_str()
                .ok_or_else(|| "Expected string for Felt252".to_string())?;
            let processed_string =
                if !string.starts_with("0x") && !string.chars().all(|c| c.is_digit(10)) {
                    // Convert to hexadecimal if it's not already hex or decimal
                    assert!(
                        string.len() <= 31,
                        "Input string must be 31 characters or less"
                    );
                    format!(
                        "0x{}",
                        string
                            .as_bytes()
                            .iter()
                            .map(|b| format!("{:02x}", b))
                            .collect::<String>()
                    )
                } else {
                    string.to_string()
                };

            Ok(vec![FuncArg::Single(
                Felt252::from_str(&processed_string).map_err(|e| e.to_string())?,
            )])
        }
        InputType::ByteArray => {
            let string = value
                .as_str()
                .ok_or_else(|| "Expected string for ByteArray".to_string())?;
            parse_byte_array(string)
        }
        InputType::Bool => {
            let bool_value = value
                .as_bool()
                .ok_or_else(|| "Expected boolean value".to_string())?;
            Ok(vec![FuncArg::Single(Felt252::from(bool_value as u64))])
        }
        InputType::Array(inner_type) => {
            let array = value
                .as_array()
                .ok_or_else(|| "Expected array".to_string())?;
            let mut result = Vec::new();
            for item in array {
                let parsed = parse_value(item, inner_type, schema)?;
                result.extend(parsed);
            }
            Ok(vec![FuncArg::Array(
                result
                    .into_iter()
                    .flat_map(|arg| match arg {
                        FuncArg::Single(felt) => vec![felt],
                        FuncArg::Array(arr) => arr,
                    })
                    .collect(),
            )])
        }
        InputType::Span(inner_type) => {
            let array = value
                .as_array()
                .ok_or_else(|| "Expected array".to_string())?;
            let mut result = Vec::new();
            for item in array {
                let parsed = parse_value(item, inner_type, schema)?;
                result.extend(parsed);
            }
            Ok(vec![FuncArg::Array(
                result
                    .into_iter()
                    .flat_map(|arg| match arg {
                        FuncArg::Single(felt) => vec![felt],
                        FuncArg::Array(arr) => arr,
                    })
                    .collect(),
            )])
        }

        InputType::Struct(struct_name) => {
            parse_struct(value, struct_name, schema).map(|func_args| func_args.0)
        }
    }
}

fn parse_struct(
    value: &Value,
    struct_name: &str,
    schema: &InputSchema,
) -> Result<FuncArgs, String> {
    let obj = value
        .as_object()
        .ok_or_else(|| format!("Expected object for struct {}", struct_name))?;

    let struct_def = schema
        .structs
        .get(struct_name)
        .ok_or_else(|| format!("Struct {} not found in schema", struct_name))?;

    let mut args = Vec::new();

    for (field_name, field_type) in &struct_def.fields {
        let value = obj
            .get(field_name)
            .ok_or_else(|| format!("Missing field: {} in struct {}", field_name, struct_name))?;

        let parsed = parse_value(value, field_type, schema)?;
        args.extend(parsed);
    }

    Ok(FuncArgs(args))
}

fn parse_byte_array(string: &str) -> Result<Vec<FuncArg>, String> {
    let byte_array =
        ByteArray::from_string(string).map_err(|e| format!("Error parsing ByteArray: {}", e))?;

    let mut result = Vec::new();
    let data = byte_array.data.iter().map(|b| b.felt()).collect::<Vec<_>>();
    result.push(FuncArg::Array(data));
    result.push(FuncArg::Single(byte_array.pending_word));
    result.push(FuncArg::Single(Felt252::from(
        byte_array.pending_word_len as u64,
    )));

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn create_temp_file_with_content(content: &str) -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(content.as_bytes()).unwrap();
        file
    }

    #[test]
    fn test_parse_input_schema_and_process_json_args() {
        // Create a temporary input schema file
        let input_schema = r#"
        Input {
            a: u32
            b: felt252
            c: Array<i32>
            d: Span<NestedStruct>
            e: ByteArray
            f: AnotherNestedStruct
            g: bool
            h: f64
        }

        NestedStruct {
            a: u32
            b: i32
            c: felt252
            d: ByteArray
        }

        AnotherNestedStruct {
            a: u32
            b: i64
        }
        "#;

        let schema_file = create_temp_file_with_content(input_schema);
        let input_schema = parse_input_schema(&schema_file.path().to_path_buf()).unwrap();

        // Create JSON input
        let json = r#"
        {
            "a": 42,
            "b": "0x68656c6c6f",
            "c": [1, -2, 3],
            "d": [
                {
                    "a": 10,
                    "b": -20,
                    "c": "30",
                    "d": "ABCD"
                },
                {
                    "a": 40,
                    "b": -50,
                    "c": "-60",
                    "d": "ABCDEFGHIJKLMNOPQRSTUVWXYZ12345"
                }
            ],
            "e": "Hello world, how are you doing today?",
            "f": {
                "a": 1,
                "b": 2
            },
            "g": true,
            "h": 0.5
        }"#;

        let result = process_json_args(json, &input_schema).unwrap();

        // Assertions
        assert_eq!(result.0.len(), 11);
        assert_eq!(result.0[0], FuncArg::Single(Felt252::from(42)));
        assert_eq!(
            result.0[1],
            FuncArg::Single(Felt252::from_str("0x68656c6c6f").unwrap())
        );
        assert_eq!(
            result.0[2],
            FuncArg::Array(vec![
                Felt252::from(1),
                Felt252::from(-2i64),
                Felt252::from(3)
            ])
        );
        assert_eq!(
            result.0[3],
            FuncArg::Array(vec![
                Felt252::from(10),
                Felt252::from(-20i64),
                Felt252::from(30),
                Felt252::from_hex(
                    "0x0000000000000000000000000000000000000000000000000000000041424344"
                )
                .unwrap(),
                Felt252::from(4),
                Felt252::from(40),
                Felt252::from(-50i64),
                Felt252::from(-60i64),
                Felt252::from_hex(
                    "0x004142434445464748494a4b4c4d4e4f505152535455565758595a3132333435"
                )
                .unwrap(),
                Felt252::from(0),
                Felt252::from(0),
            ])
        );
        assert_eq!(
            result.0[4],
            FuncArg::Array(vec![Felt252::from_hex(
                "0x48656c6c6f20776f726c642c20686f772061726520796f7520646f696e6720"
            )
            .unwrap()])
        );
        assert_eq!(
            result.0[5],
            FuncArg::Single(Felt252::from_hex("0x746f6461793f").unwrap())
        );
        assert_eq!(
            result.0[6],
            FuncArg::Single(Felt252::from_hex("0x6").unwrap())
        );
        assert_eq!(result.0[7], FuncArg::Single(Felt252::from(1)));
        assert_eq!(result.0[8], FuncArg::Single(Felt252::from(2)));
        assert_eq!(result.0[9], FuncArg::Single(Felt252::from(1)));
        assert_eq!(
            result.0[10],
            FuncArg::Single(Felt252::from_hex("0x80000000").unwrap())
        );
    }
}
