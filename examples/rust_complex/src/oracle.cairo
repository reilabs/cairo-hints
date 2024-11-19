use starknet::testing::cheatcode;
#[generate_trait]
pub impl ShirtsOracle of ShirtsOracleTrait {
    fn shirt(arg: super::shirts::Request) -> super::shirts::Response {
        let mut serialized = ArrayTrait::new();
        arg.serialize(ref serialized);
        let mut result = cheatcode::<'shirt'>(serialized.span());
        Serde::deserialize(ref result).unwrap()
    }
}
