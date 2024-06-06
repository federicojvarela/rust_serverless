/// This is used to be able to use async fixtures marked with #[once] macro. If this macro is used
/// in a fixture, the test that uses the fixture must use `#[tokio::test(flavor = "multi_thread")]`.
#[macro_export]
macro_rules! block_on {
    ($async_expr:expr) => {{
        tokio::task::block_in_place(|| {
            let handle = tokio::runtime::Handle::current();
            handle.block_on($async_expr)
        })
    }};
}
