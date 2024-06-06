use ethers::types::{Address, Bloom, Log, OtherFields, TransactionReceipt, H256, U256, U64};
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransactionResult {
    pub transaction_hash: H256,
    pub transaction_index: U64,
    pub block_hash: Option<H256>,
    pub block_number: Option<U64>,
    pub from: Address,
    pub to: Option<Address>,
    pub cumulative_gas_used: U256,
    pub gas_used: Option<U256>,
    pub contract_address: Option<Address>,
    pub logs: Vec<Log>,
    pub status: Option<U64>,
    pub transaction_type: Option<U64>,
    pub effective_gas_price: Option<U256>,

    // TODO: review need for the fields below
    pub logs_bloom: Bloom,
    pub root: Option<H256>,
    pub other: OtherFields,
}

impl From<TransactionReceipt> for TransactionResult {
    fn from(value: TransactionReceipt) -> Self {
        TransactionResult {
            transaction_hash: value.transaction_hash,
            transaction_index: value.transaction_index,
            block_hash: value.block_hash,
            block_number: value.block_number,
            from: value.from,
            to: value.to,
            cumulative_gas_used: value.cumulative_gas_used,
            gas_used: value.gas_used,
            contract_address: value.contract_address,
            logs: value.logs,
            status: value.status,
            transaction_type: value.transaction_type,
            effective_gas_price: value.effective_gas_price,
            logs_bloom: value.logs_bloom,
            root: value.root,
            other: value.other,
        }
    }
}
