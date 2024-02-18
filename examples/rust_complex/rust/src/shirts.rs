#[derive(serde::Deserialize, serde::Serialize)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Request {
    #[prost(message, optional, tag = "1")]
    pub inner: ::core::option::Option<request::Inner>,
}
/// Nested message and enum types in `Request`.
pub mod request {
    #[derive(serde::Deserialize, serde::Serialize)]
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct Inner {
        #[prost(enumeration = "super::Size", tag = "1")]
        pub color: i32,
    }
}
#[derive(serde::Deserialize, serde::Serialize)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Response {
    #[prost(enumeration = "Size", tag = "1")]
    pub color: i32,
}
#[derive(serde::Deserialize, serde::Serialize)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum Size {
    Small = 0,
    Medium = 1,
    Large = 2,
}
impl Size {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            Size::Small => "SMALL",
            Size::Medium => "MEDIUM",
            Size::Large => "LARGE",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "SMALL" => Some(Self::Small),
            "MEDIUM" => Some(Self::Medium),
            "LARGE" => Some(Self::Large),
            _ => None,
        }
    }
}
