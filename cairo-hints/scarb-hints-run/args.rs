use cainome_cairo_serde::ByteArray;
use cairo_oracle_hint_processor::{FuncArg, FuncArgs};
use cairo_vm::Felt252;
use serde_json::Value;
use std::{collections::BTreeMap, str::FromStr};

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

/// Parse a JSON string into `FuncArgs`.
/// Returns an error message if parsing fails or if the format is incorrect.
pub fn process_json_args(json_str: &str) -> Result<FuncArgs, String> {
    let json: Value =
        serde_json::from_str(json_str).map_err(|e| format!("Failed to parse JSON: {}", e))?;
    let obj = json
        .as_object()
        .ok_or_else(|| "JSON input must be an object".to_string())?;

    let mut sorted_args = BTreeMap::new();
    for (key, value) in obj {
        let parts: Vec<&str> = key.split('_').collect();
        if parts.len() != 2 {
            return Err(format!("Invalid key format: {}", key));
        }
        let index: usize = parts[0]
            .parse()
            .map_err(|_| format!("Invalid index in key: {}", key))?;
        sorted_args.insert(index, (parts[1], value));
    }

    let args = sorted_args
        .into_iter()
        .map(|(_, (ty, value))| parse_value(ty, value))
        .collect::<Result<Vec<_>, _>>()
        .map(|vec| vec.into_iter().flatten().collect());

    let args = args.map(FuncArgs);

    args
}

/// Parse a `Value` based on the specified type and return the corresponding `FuncArg`.
fn parse_value(ty: &str, value: &Value) -> Result<Vec<FuncArg>, String> {
    match ty {
        "u8" | "i8" | "u16" | "i16" | "u32" | "i32" | "u64" | "i64" => {
            let num = value
                .as_i64()
                .ok_or_else(|| format!("Expected integer for type {}", ty))?;
            Ok(vec![FuncArg::Single(Felt252::from(num))])
        }
        "felt252" => {
            let string = value
                .as_str()
                .ok_or_else(|| format!("Expected string for type {}", ty))?;
            Ok(vec![FuncArg::Single(Felt252::from_str(string).unwrap())])
        }
        ty if ty.starts_with("vec<") || ty.starts_with("span<") => parse_vector(ty, value),
        "struct" => parse_struct(value),
        "bytearray" => parse_byte_array(value, ty),
        _ => Err(format!("Unsupported type: {}", ty)),
    }
}

/// Helper function to parse vector values.
fn parse_vector(ty: &str, value: &Value) -> Result<Vec<FuncArg>, String> {
    let inner_ty = &ty[ty.find('<').unwrap() + 1..ty.len() - 1];
    let arr = value
        .as_array()
        .ok_or_else(|| format!("Expected array for type {}", ty))?;

    let parsed_results: Result<Vec<Vec<FuncArg>>, String> =
        arr.iter().map(|v| parse_value(inner_ty, v)).collect();

    let flat_parsed: Result<Vec<FuncArg>, String> =
        parsed_results.map(|vecs| vecs.into_iter().flatten().collect());

    let parsed: Result<Vec<Felt252>, String> = flat_parsed.map(|args| {
        args.into_iter()
            .flat_map(|arg| match arg {
                FuncArg::Single(felt) => vec![felt],
                FuncArg::Array(arr) => arr,
            })
            .collect()
    });

    parsed.map(|result| vec![FuncArg::Array(result)])
}

/// Helper function to parse struct values.
fn parse_struct(value: &Value) -> Result<Vec<FuncArg>, String> {
    let obj = value
        .as_object()
        .ok_or_else(|| "Expected object for struct".to_string())?;
    let mut results = Vec::new();
    for (key, val) in obj {
        let parts: Vec<&str> = key.split('_').collect();
        if parts.len() != 2 {
            return Err(format!("Invalid struct field format: {}", key));
        }
        let field_value = parse_value(parts[1], val)?;
        results.extend(field_value);
    }
    Ok(results)
}

/// Helper function to parse byte array values.
fn parse_byte_array(value: &Value, ty: &str) -> Result<Vec<FuncArg>, String> {
    // Fixed typo, passed ty
    let string = value
        .as_str()
        .ok_or_else(|| format!("Expected string for type {}", ty))?;

    match ByteArray::from_string(string) {
        Ok(byte_array) => {
            let mut result = Vec::new();
            let data = byte_array.data.iter().map(|b| b.felt()).collect::<Vec<_>>();
            result.push(FuncArg::Array(data));
            result.push(FuncArg::Single(byte_array.pending_word));
            result.push(FuncArg::Single(Felt252::from(
                byte_array.pending_word_len as i64,
            )));
            Ok(result)
        }
        Err(e) => Err(format!("Error parsing bytearray: {}", e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_json_to_funcargs() {
        let json = r#"
        {
            "0_u32": 42,
            "1_felt252": "0x68656c6c6f",
            "2_vec<i32>": [1, -2, 3],
            "3_vec<struct>": [
                {
                    "0_u32": 10,
                    "1_i32": -20,
                    "2_felt252": "30",
                    "3_bytearray": "ABCD"
                },
                {
                    "0_u32": 40,
                    "1_i32": -50,
                    "2_felt252": "-60",
                    "3_bytearray": "ABCDEFGHIJKLMNOPQRSTUVWXYZ12345"
                }
            ],
            "4_bytearray": "Hello world, how are you doing today?",
            "5_struct" : {
                "0_u32": 1,
                "1_i64": 2
            }
        }"#;

        let result = process_json_args(json).unwrap();

        assert_eq!(result.0.len(), 9);
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
    }
}
