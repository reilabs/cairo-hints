#[derive(serde::Deserialize, serde::Serialize)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Request {
    #[prost(uint32, tag = "1")]
    pub n: u32,
    #[prost(uint32, tag = "2")]
    pub len: u32,
}
#[derive(serde::Deserialize, serde::Serialize)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Response {
    #[prost(uint32, repeated, tag = "1")]
    pub nb: ::prost::alloc::vec::Vec<u32>,
}
