use extern_trait::extern_trait;

#[extern_trait(ResourceProxy)]
#[allow(clippy::missing_safety_doc)]
unsafe trait Resource {
    fn new() -> Self;
    fn count(&self) -> usize;
}

mod resource_impl {
    use std::sync::{Arc, LazyLock};

    use super::*;

    static GLOBAL: LazyLock<Arc<()>> = LazyLock::new(|| Arc::new(()));

    struct ActualResource(Arc<()>);

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
    let res = ResourceProxy::new();
    assert_eq!(res.count(), 2);

    {
        let res = ResourceProxy::new();
        assert_eq!(res.count(), 3);
    }

    assert_eq!(res.count(), 2);
}
