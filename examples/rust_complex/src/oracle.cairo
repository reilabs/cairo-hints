use starknet::testing::cheatcode;
#[derive(Drop, Serde)]
struct Request {
    n: u32,
    len: u32,
}
#[derive(Drop, Serde)]
struct Response {
    nb: Array<u32>,
}
#[generate_trait]
impl BinaryOracle of BinaryOracleTrait {
    fn to_binary(arg: super::oracle::Request) -> super::oracle::Response {
        let mut serialized = ArrayTrait::new();
        arg.serialize(ref serialized);
        let mut result = cheatcode::<'to_binary'>(serialized.span());
        Serde::deserialize(ref result).unwrap()
    }
}
