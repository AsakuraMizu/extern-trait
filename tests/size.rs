use extern_trait::extern_trait;

#[extern_trait(TupleProxy)]
#[allow(clippy::missing_safety_doc)]
unsafe trait Tuple {
    fn new(a: usize, b: usize) -> Self;
    fn get(&self) -> (usize, usize);
}

mod tuple_impl {
    use super::*;

    struct TupleImpl(usize, usize);

    #[extern_trait]
    unsafe impl Tuple for TupleImpl {
        fn new(a: usize, b: usize) -> Self {
            Self(a, b)
        }

        fn get(&self) -> (usize, usize) {
            (self.0, self.1)
        }
    }
}

#[test]
fn test_tuple() {
    let tuple = TupleProxy::new(1, 2);
    let (a, b) = tuple.get();
    assert_eq!(a, 1);
    assert_eq!(b, 2);
}
