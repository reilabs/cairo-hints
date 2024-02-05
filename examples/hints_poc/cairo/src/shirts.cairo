use starknet::testing::cheatcode;
#[derive(Drop, Serde, PartialEq)]
enum Size {
    Small,
    Medium,
    Large,
}
