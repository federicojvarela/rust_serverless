pub mod fts;
pub mod nfts;

use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct AlchemyRPCResponse<T> {
    #[allow(dead_code)]
    id: String,

    #[allow(dead_code)]
    jsonrpc: String,

    result: T,
}

impl<T> AlchemyRPCResponse<T> {
    pub fn into_inner(self) -> T {
        self.result
    }
}
