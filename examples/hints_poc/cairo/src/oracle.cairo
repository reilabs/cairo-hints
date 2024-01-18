use starknet::testing::cheatcode;

trait Sendable<T> {
    fn send(self: @T);
    fn recv() -> T;
}

// TODO: missing implementation for builtin types
impl Sendableu64 of Sendable<u64> {
    fn send(self: @u64) {
        let val: felt252 = (*self).into();
        cheatcode::<'oracle_value_push'>(array!['u64', val].span());
    }
    fn recv() -> u64 {
        let mut bytes = cheatcode::<'oracle_value_pop'>(array!['u64'].span()); // could enforce type here!
        Serde::<u64>::deserialize(ref bytes).unwrap()
    }
}

impl Sendableu32 of Sendable<u32> {
    fn send(self: @u32) {
        let val: felt252 = (*self).into();
        cheatcode::<'oracle_value_push'>(array!['u32', val].span());
    }
    fn recv() -> u32 {
        let mut bytes = cheatcode::<'oracle_value_pop'>(array!['u32'].span()); // could enforce type here!
        Serde::<u32>::deserialize(ref bytes).unwrap()
    }
}

impl Sendablei32 of Sendable<i32> {
    fn send(self: @i32) {
        let val: felt252 = (*self).into();
        cheatcode::<'oracle_value_push'>(array!['i32', val].span());
    }
    fn recv() -> i32 {
        let mut bytes = cheatcode::<'oracle_value_pop'>(array!['i32'].span()); // could enforce type here!
        Serde::<i32>::deserialize(ref bytes).unwrap()
    }
}

impl optionimpl<T, +Sendable<T>> of Sendable<Option<T>> {
    fn send(self: @Option<T>) {
        cheatcode::<'oracle_path_push'>(array!['struct'].span());

        match self {
            Option::Some(v) => {
                cheatcode::<'oracle_key_push'>(array!['presence'].span());
                Sendable::<u64>::send(@1); // present
                cheatcode::<'oracle_key_pop'>(array!['presence'].span());

                cheatcode::<'oracle_key_push'>(array!['value'].span());
                Sendable::<T>::send(v); // present
                cheatcode::<'oracle_key_pop'>(array!['value'].span());
            },
            Option::None => {
                cheatcode::<'oracle_key_push'>(array!['presence'].span());
                Sendable::<u64>::send(@0); // not present
                cheatcode::<'oracle_key_pop'>(array!['presence'].span());
            }
        }

        cheatcode::<'oracle_path_pop'>(array!['struct'].span());
    }
    fn recv() -> Option<T> {
        Option::None
    }
}

impl ArraySendable<T, +Sendable<T>> of Sendable<Array<T>> {
    fn send(self: @Array<T>) {
        cheatcode::<'oracle_path_push'>(array!['array'].span());
        let mut i: usize = 0;
        loop {
            if i >= self.len() {
                break;
            }
            self.at(i).send();
        };
        cheatcode::<'oracle_path_pop'>(array!['array'].span());
    }

    fn recv() -> Array<T> {
        array![]
    }
}

impl ByteArraySendable of Sendable<ByteArray> {
    fn send(self: @ByteArray) {
    }

    fn recv() -> ByteArray {
        Default::default()
    }
}

#[derive(Serde, Drop)]
struct Inner {
    inner: u32,
}
impl SendableInner of Sendable<Inner> {
    fn send(self: @Inner) {
        cheatcode::<'oracle_path_push'>(array!['struct'].span());
        cheatcode::<'oracle_key_push'>(array!['inner'].span());
        self.inner.send();
        cheatcode::<'oracle_key_pop'>(array!['inner'].span());
        cheatcode::<'oracle_path_pop'>(array!['struct'].span());
    }
    fn recv() -> Inner {
        cheatcode::<'oracle_path_push'>(array!['struct'].span());
        cheatcode::<'oracle_key_push'>(array!['inner'].span());
        let inner = Sendable::<u32>::recv();
        cheatcode::<'oracle_key_pop'>(array!['inner'].span());
        cheatcode::<'oracle_path_pop'>(array!['struct'].span());
        Inner { inner }
    }
}
#[derive(Serde, Drop)]
struct Request {
    n: u64,
    x: Option<Inner>,
    y: Array<i32>,
}
impl SendableRequest of Sendable<Request> {
    fn send(self: @Request) {
        cheatcode::<'oracle_path_push'>(array!['struct'].span());
        cheatcode::<'oracle_key_push'>(array!['n'].span());
        self.n.send();
        cheatcode::<'oracle_key_pop'>(array!['n'].span());
        cheatcode::<'oracle_key_push'>(array!['x'].span());
        self.x.send();
        cheatcode::<'oracle_key_pop'>(array!['x'].span());
        cheatcode::<'oracle_key_push'>(array!['y'].span());
        self.y.send();
        cheatcode::<'oracle_key_pop'>(array!['y'].span());
        cheatcode::<'oracle_path_pop'>(array!['struct'].span());
    }
    fn recv() -> Request {
        cheatcode::<'oracle_path_push'>(array!['struct'].span());
        cheatcode::<'oracle_key_push'>(array!['n'].span());
        let n = Sendable::<u64>::recv();
        cheatcode::<'oracle_key_pop'>(array!['n'].span());
        cheatcode::<'oracle_key_push'>(array!['x'].span());
        let x = Sendable::<Option<Inner>>::recv();
        cheatcode::<'oracle_key_pop'>(array!['x'].span());
        cheatcode::<'oracle_key_push'>(array!['y'].span());
        let y = Sendable::<Array<i32>>::recv();
        cheatcode::<'oracle_key_pop'>(array!['y'].span());
        cheatcode::<'oracle_path_pop'>(array!['struct'].span());
        Request { n, x, y }
    }
}
#[derive(Serde, Drop)]
struct Response {
    n: u64,
}
impl SendableResponse of Sendable<Response> {
    fn send(self: @Response) {
        cheatcode::<'oracle_path_push'>(array!['struct'].span());
        cheatcode::<'oracle_key_push'>(array!['n'].span());
        self.n.send();
        cheatcode::<'oracle_key_pop'>(array!['n'].span());
        cheatcode::<'oracle_path_pop'>(array!['struct'].span());
    }
    fn recv() -> Response {
        cheatcode::<'oracle_path_push'>(array!['struct'].span());
        cheatcode::<'oracle_key_push'>(array!['n'].span());
        let n = Sendable::<u64>::recv();
        cheatcode::<'oracle_key_pop'>(array!['n'].span());
        cheatcode::<'oracle_path_pop'>(array!['struct'].span());
        Response { n }
    }
}
#[generate_trait]
impl SqrtOracle of SqrtOracleTrait {
    fn sqrt(arg: Request) -> Response {
                arg.send();
                cheatcode::<'oracle_ask'>(array!['sqrt'].span());
                Sendable::<Response>::recv()
    }
}
