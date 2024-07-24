use std::{ops::Deref, str::FromStr};

use cairo_lang_runner::Arg;
use cairo_vm::Felt252;
use serde::{de::Visitor, Deserialize};
use serde_json::Value;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ArgsError {
    #[error("failed to parse number: {0}")]
    NumberParseError(#[from] std::num::ParseIntError),
    #[error("failed to parse bigint: {0}")]
    BigIntParseError(#[from] num_bigint::ParseBigIntError),
    #[error("number out of range")]
    NumberOutOfRange,
    #[error("failed to parse arguments: {0}")]
    ParseError(String),
}

/// `Args` is a wrapper around a vector of `Arg`.
///
/// It provides convenience methods for working with a vector of `Arg` and implements
/// `Deref` to allow it to be treated like a vector of `Arg`.
#[derive(Debug)]
pub struct Args(Vec<Arg>);

impl Args {
    /// Creates a new `Args` from a vector of `Arg`.
    ///
    /// # Arguments
    ///
    /// * `args` - A vector of `Arg`.
    ///
    /// # Returns
    ///
    /// * `Args` - A new `Args` instance.
    #[must_use]
    pub fn new(args: Vec<Arg>) -> Self {
        Self(args)
    }

    fn clone_arg(arg: &Arg) -> Arg {
        match arg {
            Arg::Value(v) => Arg::Value(*v),
            Arg::Array(arr) => Arg::Array(arr.iter().map(Self::clone_arg).collect()),
        }
    }
}

impl Clone for Args {
    fn clone(&self) -> Self {
        Self(self.0.iter().map(Self::clone_arg).collect())
    }
}

impl Deref for Args {
    type Target = Vec<Arg>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<Args> for Vec<Arg> {
    fn from(args: Args) -> Self {
        args.0
    }
}

impl From<Vec<Arg>> for Args {
    fn from(args: Vec<Arg>) -> Self {
        Self(args)
    }
}

impl FromStr for Args {
    type Err = ArgsError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let args =
            serde_json::from_str::<Args>(s).map_err(|e| ArgsError::ParseError(e.to_string()))?;
        Ok(args)
    }
}

impl Args {
    fn visit_seq_helper(seq: &[Value]) -> Result<Self, ArgsError> {
        let iterator = seq.iter();
        let mut args = Vec::new();

        for arg in iterator {
            match arg {
                Value::Number(n) => {
                    let n = n.as_u64().ok_or(ArgsError::NumberOutOfRange)?;
                    args.push(Arg::Value(Felt252::from(n)));
                }
                Value::String(n) => {
                    let n = num_bigint::BigUint::from_str(n)?;
                    let bytes = n.to_bytes_be();
                    let mut padded = [0u8; 32];
                    padded[32 - bytes.len()..].copy_from_slice(&bytes);
                    args.push(Arg::Value(Felt252::from_bytes_be(&padded)));
                }
                Value::Array(arr) => {
                    let inner_args =
                        arr.iter()
                            .map(|a| match a {
                                Value::Number(n) => {
                                    let n = n.as_u64().ok_or(ArgsError::NumberOutOfRange)?;
                                    Ok(Arg::Value(Felt252::from(n)))
                                }
                                Value::String(n) => {
                                    let n = num_bigint::BigUint::from_str(n)?;
                                    let bytes = n.to_bytes_be();
                                    let mut padded = [0u8; 32];
                                    padded[32 - bytes.len()..].copy_from_slice(&bytes);
                                    Ok(Arg::Value(Felt252::from_bytes_be(&padded)))
                                }
                                Value::Array(nested_arr) => Self::visit_seq_helper(nested_arr)
                                    .map(|args| Arg::Array(args.0)),
                                _ => Err(ArgsError::ParseError("Invalid type".to_string())),
                            })
                            .collect::<Result<Vec<Arg>, ArgsError>>()?;
                    args.push(Arg::Array(inner_args));
                }
                _ => return Err(ArgsError::ParseError("Invalid type".to_string())),
            }
        }

        Ok(Self::new(args))
    }
}

impl<'de> Visitor<'de> for Args {
    type Value = Args;
    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a list of arguments")
    }
    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::SeqAccess<'de>,
    {
        let mut args = Vec::new();
        while let Some(arg) = seq.next_element()? {
            match arg {
                Value::Number(n) => args.push(Value::Number(n)),
                Value::String(n) => args.push(Value::String(n)),
                Value::Array(a) => args.push(Value::Array(a)),
                _ => return Err(serde::de::Error::custom("Invalid type")),
            }
        }

        Self::visit_seq_helper(&args).map_err(|e| serde::de::Error::custom(e.to_string()))
    }
}

impl<'de> Deserialize<'de> for Args {
    fn deserialize<D>(deserializer: D) -> Result<Args, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_seq(Args(Vec::new()))
    }
}
