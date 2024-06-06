use ethers::types::{Address, U256};
use serde::Deserialize;

use common::deserializers::{h160::h160, u256::unsigned_integer_256};

#[derive(Deserialize)]
pub struct AlchemyGetFTsResponse {
    pub result: AlchemyFTsResult,
}

#[derive(Deserialize)]
pub struct AlchemyGetMetadataResponse {
    pub result: AlchemyFTTokenMetadata,
}

#[derive(Deserialize)]
pub struct AlchemyGetMetadataErrorResponse {
    pub error: AlechmyErrorResponse,
}

#[derive(Deserialize)]
pub struct AlechmyErrorResponse {
    pub code: i32,
    pub message: String,
}

#[derive(Deserialize)]
pub struct AlchemyFTTokenMetadata {
    pub decimals: Option<u32>,
    pub logo: Option<String>,
    pub name: String,
    pub symbol: String,
}

#[derive(Deserialize)]
pub struct AlchemyFTsResult {
    #[serde(deserialize_with = "h160")]
    pub address: Address,
    #[serde(rename(deserialize = "tokenBalances"))]
    pub token_balances: Vec<AlchemyFTTokenBalance>,
}

#[derive(Deserialize)]
pub struct AlchemyFTTokenBalance {
    #[serde(rename(deserialize = "contractAddress"))]
    pub contract_address: String,
    #[serde(
        rename(deserialize = "tokenBalance"),
        deserialize_with = "unsigned_integer_256"
    )]
    pub token_balance: U256,
}
