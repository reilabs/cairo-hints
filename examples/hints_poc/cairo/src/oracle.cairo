use starknet::testing::cheatcode;
#[generate_trait]
impl SqrtOracle of SqrtOracleTrait {
    fn sqrt(arg: super::shirts::Request) -> super::shirts::Response {
        let mut serialized = ArrayTrait::new();
        arg.serialize(ref serialized);
        let mut result = cheatcode::<'sqrt'>(serialized.span());
        Serde::deserialize(ref result).unwrap()
    }
}
