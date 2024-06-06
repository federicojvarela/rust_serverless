use ethers::types::H256;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
pub enum MaestroSignResponse {
    Approved {
        #[serde(rename(deserialize = "rlp_encoded_signed_transaction"))]
        signature: String,
        transaction_hash: H256,
    },
    Rejected {
        reason: String,
    },
}
