use starknet::testing::cheatcode;
#[derive(Drop, Serde)]
struct Request {
    inner: Option<super::shirts::request::Inner>,
}
/// Nested message and enum types in `Request`.
mod request {
    #[derive(Drop, Serde)]
    struct Inner {
        color: super::super::shirts::Size,
    }
}
#[derive(Drop, Serde)]
struct Response {
    color: super::shirts::Size,
}
#[derive(Drop, Serde, PartialEq)]
enum Size {
    Small,
    Medium,
    Large,
}
