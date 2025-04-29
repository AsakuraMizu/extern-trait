use extern_trait::extern_trait;

#[extern_trait(StaticImpl)]
#[allow(clippy::missing_safety_doc)]
unsafe trait Static {
    fn add(a: i32, b: i32) -> i32;
    fn sub(a: i32, b: i32) -> i32;
}

mod static_impl {
    use super::*;

    struct RemoteImpl;

    #[extern_trait]
    unsafe impl Static for RemoteImpl {
        fn add(a: i32, b: i32) -> i32 {
            a + b
        }

        fn sub(a: i32, b: i32) -> i32 {
            a - b
        }
    }
}

#[test]
fn test_static() {
    assert_eq!(StaticImpl::add(1, 2), 3);
    assert_eq!(StaticImpl::sub(3, 2), 1);
}
