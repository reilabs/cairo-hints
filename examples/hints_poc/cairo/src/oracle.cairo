use starknet::testing::cheatcode;
#[derive(Drop, Serde)]
struct Request {
    color: bool,
    size: Size,
}
#[derive(Drop, Serde)]
struct Response {
    color: bool,
    size: Size,
}
#[derive(Drop, Serde, PartialEq)]
enum Size {
    Small,
    Medium,
    Large,
}
#[generate_trait]
impl SqrtOracle of SqrtOracleTrait {
    fn sqrt(arg: Request) -> Response {
        let mut serialized = ArrayTrait::new();
        arg.serialize(ref serialized);
        let mut result = cheatcode::<'sqrt'>(serialized.span());
        Serde::deserialize(ref result).unwrap()
    }
}
