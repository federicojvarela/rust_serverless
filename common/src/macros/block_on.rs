#[macro_export]
macro_rules! block_on {
    ($async_expr:expr) => {{
        tokio::task::block_in_place(|| {
            let handle = tokio::runtime::Handle::current();
            handle.block_on($async_expr)
        })
    }};
}
