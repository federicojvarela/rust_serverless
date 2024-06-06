use ethers::types::TxHash;
use serde::Serialize;

#[derive(Serialize, Debug)]
pub enum SendTransactionResponse {
    Submitted { tx_hash: TxHash },
    NotSubmitted { code: i64, message: String },
}
