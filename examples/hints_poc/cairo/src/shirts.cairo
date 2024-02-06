use starknet::testing::cheatcode;
#[derive(Drop, Serde)]
struct Request {
    color: Size,
}
#[derive(Drop, Serde)]
struct Response {
    color: Size,
}
#[derive(Drop, Serde, PartialEq)]
enum Size {
    Small,
    Medium,
    Large,
}
