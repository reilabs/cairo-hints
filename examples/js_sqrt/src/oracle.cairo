use starknet::testing::cheatcode;
#[derive(Drop, Serde)]
pub struct Request {
    pub n: u64,
}
#[derive(Drop, Serde)]
pub struct Response {
    pub n: u64,
}
#[generate_trait]
pub impl SqrtOracle of SqrtOracleTrait {
    fn sqrt(arg: super::oracle::Request) -> super::oracle::Response {
        let mut serialized = ArrayTrait::new();
        arg.serialize(ref serialized);
        let mut result = cheatcode::<'sqrt'>(serialized.span());
        Serde::deserialize(ref result).unwrap()
    }
}
