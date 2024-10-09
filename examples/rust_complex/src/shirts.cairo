use starknet::testing::cheatcode;
#[derive(Drop, Serde)]
pub struct Request {
    pub inner: Option<super::shirts::request::Inner>,
}
/// Nested message and enum types in `Request`.
pub mod request {
    #[derive(Drop, Serde)]
    pub struct Inner {
        pub color: super::super::shirts::Size,
    }
}
#[derive(Drop, Serde)]
pub struct Response {
    pub color: super::shirts::Size,
}
#[derive(Drop, Serde, PartialEq)]
pub enum Size {
    Small,
    Medium,
    Large,
}
