use starknet::testing::cheatcode;
#[derive(Drop, Serde)]
struct Request {
    color: super::shirts::Size,
}
#[derive(Drop, Serde)]
struct Response {
    color: super::shirts::Size,
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
