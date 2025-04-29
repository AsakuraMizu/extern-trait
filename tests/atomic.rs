use extern_trait::extern_trait;

#[extern_trait(AtomicProxy)]
#[allow(clippy::missing_safety_doc)]
unsafe trait Atomic {
    fn new(v: i32) -> Self;
    fn get(&self) -> i32;
    fn set(&self, v: i32);
}

mod atomic_impl {
    use std::sync::atomic::{AtomicI32, Ordering};

    use super::*;

    struct AtomicImpl(AtomicI32);

    #[extern_trait]
    unsafe impl Atomic for AtomicImpl {
        fn new(v: i32) -> Self {
            Self(AtomicI32::new(v))
        }

        fn get(&self) -> i32 {
            self.0.load(Ordering::Relaxed)
        }

        fn set(&self, v: i32) {
            self.0.store(v, Ordering::Relaxed);
        }
    }
}

#[test]
fn test_atomic() {
    let atomic = AtomicProxy::new(0);
    assert_eq!(atomic.get(), 0);

    atomic.set(42);
    assert_eq!(atomic.get(), 42);

    atomic.set(100);
    assert_eq!(atomic.get(), 100);
}
