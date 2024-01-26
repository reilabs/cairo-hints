use starknet::testing::cheatcode;
#[derive(Drop, Serde)]
struct Inner {
    inner: u32,
}
#[derive(Drop, Serde)]
struct Request {
    n: u64,
    x: Option<Inner>,
    y: Array<i32>,
}
#[derive(Drop, Serde)]
struct Response {
    n: u64,
}
#[generate_trait]
impl SqrtOracle of SqrtOracleTrait {
    fn sqrt(arg: Request) -> Response {
        let mut serialized = ArrayTrait::new();
        ('sqrt', arg).serialize(ref serialized);
        let mut result = cheatcode::<'oracle_ask'>(serialized.span());
        Serde::deserialize(ref result).unwrap()
    }
}
