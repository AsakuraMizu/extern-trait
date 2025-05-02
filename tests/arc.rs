use extern_trait::extern_trait;

#[extern_trait(ResourceProxy)]
#[allow(clippy::missing_safety_doc)]
unsafe trait Resource: Send + Sync + AsRef<str> {
    fn new() -> Self;
    fn count(&self) -> usize;
}

mod resource_impl {
    use std::sync::{Arc, LazyLock};

    use super::*;

    static GLOBAL: LazyLock<Arc<String>> = LazyLock::new(|| Arc::new("Hello, world!".to_string()));

    struct ActualResource(Arc<String>);

    impl AsRef<str> for ActualResource {
        fn as_ref(&self) -> &str {
            self.0.as_ref()
        }
    }

    #[extern_trait]
    unsafe impl Resource for ActualResource {
        fn new() -> Self {
            Self(GLOBAL.clone())
        }

        fn count(&self) -> usize {
            Arc::strong_count(&self.0)
        }
    }
}

#[test]
fn test_resource() {
    struct _AssertSend
    where
        ResourceProxy: Send;
    struct _AssertSync
    where
        ResourceProxy: Sync;

    let res = ResourceProxy::new();
    assert_eq!(res.count(), 2);
    assert_eq!(res.as_ref(), "Hello, world!");

    {
        let res = ResourceProxy::new();
        assert_eq!(res.count(), 3);
    }

    assert_eq!(res.count(), 2);
}
