use starknet::testing::cheatcode;
#[derive(Drop, Serde)]
struct RequestUInt32 {
    n: u32,
}
#[derive(Drop, Serde)]
struct ResponseUInt32 {
    n: u32,
}
#[generate_trait]
impl SqrtOracle of SqrtOracleTrait {
    fn sqrt(arg: RequestUInt32) -> ResponseUInt32 {
        let mut serialized = ArrayTrait::new();
        arg.serialize(ref serialized);
        let mut result = cheatcode::<'sqrt'>(serialized.span());
        Serde::deserialize(ref result).unwrap()
    }
}
