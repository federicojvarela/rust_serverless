use crate::config::SupportedChain;
use anyhow::anyhow;
use ethers::types::Chain;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Config {
    pub ethereum_mainnet_endpoint: String,
    pub ethereum_sepolia_endpoint: String,

    pub polygon_mainnet_endpoint: String,
    pub polygon_amoy_endpoint: String,

    // None for production, Some(..) for integration testing
    pub ganache_endpoint: Option<String>,

    // None for local testing
    pub ethereum_mainnet_api_key_secret_name: Option<String>,
    pub ethereum_sepolia_api_key_secret_name: Option<String>,
    pub polygon_mainnet_api_key_secret_name: Option<String>,
    pub polygon_amoy_api_key_secret_name: Option<String>,
}

pub struct ChainEndpointInformation {
    pub endpoint: String,
    pub secret_name: Option<String>,
}

pub struct ChainNativeToken {
    pub name: String,
    pub symbol: String,
}

impl Config {
    pub fn evm_chain_endpoint_information_for(
        &self,
        chain_id: u64,
    ) -> Result<ChainEndpointInformation, anyhow::Error> {
        let chain = Chain::try_from(chain_id)
            .map_err(|e| anyhow!(e).context(format!("Invalid chain_id: {chain_id}")))?;

        if !chain.is_supported() {
            return Err(anyhow!("Network not supported {chain}"));
        }

        let endpoint_information = match chain {
            Chain::Mainnet => ChainEndpointInformation {
                endpoint: self.ethereum_mainnet_endpoint.clone(),
                secret_name: self.ethereum_mainnet_api_key_secret_name.as_ref().cloned(),
            },
            Chain::Sepolia => ChainEndpointInformation {
                endpoint: self.ethereum_sepolia_endpoint.clone(),
                secret_name: self.ethereum_sepolia_api_key_secret_name.as_ref().cloned(),
            },
            Chain::Polygon => ChainEndpointInformation {
                endpoint: self.polygon_mainnet_endpoint.clone(),
                secret_name: self.polygon_mainnet_api_key_secret_name.as_ref().cloned(),
            },
            Chain::PolygonAmoy => ChainEndpointInformation {
                endpoint: self.polygon_amoy_endpoint.clone(),
                secret_name: self.polygon_amoy_api_key_secret_name.as_ref().cloned(),
            },
            Chain::Dev => {
                if let Some(endpoint) = &self.ganache_endpoint {
                    ChainEndpointInformation {
                        endpoint: endpoint.to_owned(),
                        secret_name: None,
                    }
                } else {
                    Err(anyhow!("Dev chain found but no endpoint was specified"))?
                }
            }
            other => Err(anyhow!("Supported chain NOT processed! {other}"))?,
        };

        Ok(endpoint_information)
    }

    pub fn get_chain_native_token(&self, chain_id: u64) -> Result<ChainNativeToken, anyhow::Error> {
        let chain = Chain::try_from(chain_id)
            .map_err(|e| anyhow!(e).context(format!("Invalid chain_id: {chain_id}")))?;

        if !chain.is_supported() {
            return Err(anyhow!("Network not supported {chain}"));
        }

        let native_token = match chain {
            Chain::Mainnet => ChainNativeToken {
                name: "Ether".to_string(),
                symbol: "ETH".to_string(),
            },
            Chain::Sepolia => ChainNativeToken {
                name: "Sepolia Ether".to_string(),
                symbol: "ETH".to_string(),
            },
            Chain::Polygon => ChainNativeToken {
                name: "Polygon".to_string(),
                symbol: "MATIC".to_string(),
            },
            Chain::PolygonAmoy => ChainNativeToken {
                name: "Amoy Polygon".to_string(),
                symbol: "MATIC".to_string(),
            },
            Chain::Dev => ChainNativeToken {
                name: "Ether".to_string(),
                symbol: "ETH".to_string(),
            },
            other => Err(anyhow!("Supported chain NOT processed! {other}"))?,
        };

        Ok(native_token)
    }
}
