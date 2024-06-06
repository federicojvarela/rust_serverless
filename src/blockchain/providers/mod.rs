pub mod alchemy;

use async_trait::async_trait;
use common::deserializers::u64::str_u64;
use ethers::types::{Address, Transaction, U256};
use serde::{Deserialize, Serialize};

type Result<T> = std::result::Result<T, BlockchainProviderError>;

#[derive(Debug, thiserror::Error)]
pub enum BlockchainProviderError {
    #[error("{0:#}")]
    Unknown(anyhow::Error),
}

#[async_trait]
#[cfg_attr(test, mockall::automock)]
pub trait EvmBlockchainProvider: Sync + Send {
    async fn get_evm_endpoint(
        &self,
        chain_id: u64,
        endpoint_prefix: Option<String>,
    ) -> Result<String>;

    async fn get_native_token_info(
        &self,
        chain_id: u64,
        address: Address,
    ) -> Result<NativeTokenInfo>;

    async fn get_non_fungible_token_info(
        &self,
        chain_id: u64,
        address: Address,
        contract_addresses: Vec<Address>,
        pagination: Pagination,
    ) -> Result<NonFungibleTokenInfo>;

    async fn get_fungible_token_info(
        &self,
        chain_id: u64,
        address: Address,
        contract_addresses: Vec<Address>,
    ) -> Result<FungibleTokenInfo>;

    async fn get_fungible_token_metadata(
        &self,
        chain_id: u64,
        address: Address,
    ) -> Result<FungibleTokenMetadataInfo>;

    /// Gets the historical information of gas spent from `newest_bock` to `newest_block` -
    /// `block_count` from an specific chain id.
    ///
    /// # Arguments
    /// - `chain_id` - The chain id.
    /// - `block_count` - Number of blocks requested.
    /// - `newest_block` - Highest number of block requested in the range. `NewestBlock::Lastest`
    /// represents the last mined block.
    /// - `reward_percentiles` - An increasing array of percentiles to sample each block priority
    /// fees.
    ///
    /// # Returns
    /// - `oldest_block` - The lowest block number returned.
    /// - `base_fee_per_gas` - An array of block base fee per gas.
    /// - `gas_used_ratio` - An array of gas used ratio.
    /// - `rewards`- An array containing the effective priority fees per gas by percentiles.
    ///
    /// # Example
    /// ```rust,ignore
    /// // Getting fee history from the latest block, five blocks in Sepolia
    /// alchemy_client
    ///      .get_fee_history(11155111, 5, NewestBlock::Latest, &[0.25, 0.5, 0.75])
    ///      .await
    /// ```
    ///
    /// For more information:
    /// https://ethereum.github.io/execution-apis/api-documentation/
    async fn get_fee_history<'percentiles>(
        &self,
        chain_id: u64,
        block_count: u64,
        newest_block: NewestBlock,
        reward_percentiles: &'percentiles [f64],
    ) -> Result<FeeHistory>;

    async fn tx_status_succeed(&self, chain_id: u64, tx_hash: String) -> Result<bool>;

    async fn get_tx_by_hash(&self, chain_id: u64, tx_hash: String) -> Result<Option<Transaction>>;

    async fn get_fees_from_pending(&self, chain_id: u64) -> Result<BlockFeeQuery>;

    async fn get_tx_receipt(
        &self,
        chain_id: u64,
        tx_hash: String,
    ) -> Result<Option<ethers::types::TransactionReceipt>>;

    async fn get_current_nonce(&self, chain_id: u64, address: Address) -> Result<U256>;
}

pub enum NewestBlock {
    BlockNumber(u64),
    Latest,
}

#[derive(Deserialize, Debug)]
pub struct FeeHistory {
    #[serde(deserialize_with = "str_u64", rename(deserialize = "oldestBlock"))]
    pub oldest_block: u64,

    #[serde(rename(deserialize = "baseFeePerGas"))]
    pub base_fee_per_gas: Vec<U256>,

    #[serde(rename(deserialize = "gasUsedRatio"))]
    pub gas_used_ratio: Vec<f64>,

    pub reward: Vec<Vec<U256>>,
}

#[derive(Deserialize, Debug)]
pub struct BlockFeeQuery {
    pub max_priority_fees: Vec<U256>,
    pub base_fee_per_gas: U256,
}

#[derive(Deserialize, Debug)]
pub struct MaxPriorityFeePerGasEstimates {
    pub low: U256,
    pub medium: U256,
    pub high: U256,
}

#[derive(Serialize)]
pub struct NativeTokenInfo {
    pub name: String,
    pub symbol: String,
    pub chain_id: u64,
    pub balance: String,
}

#[derive(Serialize)]
pub struct NonFungibleTokenInfo {
    pub tokens: Vec<NonFungibleTokenInfoDetail>,
    pub pagination: Pagination,
}

#[derive(Serialize)]
pub struct NonFungibleTokenInfoDetail {
    pub contract_address: Address,
    pub name: String,
    pub symbol: String,
    pub balance: String,
    pub metadata: NonFungibleTokenInfoMetadata,
}

#[derive(Serialize)]
pub struct NonFungibleTokenInfoMetadata {
    pub name: String,
    pub description: String,
    pub image: String,
    pub attributes: Vec<NonFungibleTokenInfoAttribute>,
}

#[derive(Serialize)]
pub struct NonFungibleTokenInfoAttribute {
    pub value: String,
    pub trait_type: String,
}

#[derive(Serialize)]
pub struct FungibleTokenInfo {
    pub data: Vec<FungibleTokenInfoDetail>,
    pub errors: Vec<TokenError>,
}

#[derive(Serialize)]
pub struct FungibleTokenMetadataInfo {
    pub data: Option<FungibleTokenMetadata>,
    pub error: Option<TokenError>,
}

#[derive(Serialize)]
pub struct TokenError {
    pub contract_address: Address,
    pub reason: String,
}

#[derive(Serialize)]
pub struct FungibleTokenInfoDetail {
    pub contract_address: Address,
    pub balance: String,
    pub name: String,
    pub symbol: String,
    pub logo: String,
    pub decimals: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct FungibleTokenMetadata {
    pub name: String,
    pub symbol: String,
    pub logo: String,
    pub decimals: String,
}

pub const PAGE_SIZE_DEFAULT: u32 = 10;

#[derive(Serialize)]
pub struct Pagination {
    pub page_size: u32,
    pub page_key: Option<String>,
}

impl Default for Pagination {
    fn default() -> Self {
        Self {
            page_size: PAGE_SIZE_DEFAULT,
            page_key: None,
        }
    }
}
