use crate::blockchain::providers::{
    NonFungibleTokenInfoAttribute, NonFungibleTokenInfoDetail, NonFungibleTokenInfoMetadata,
};
use anyhow::anyhow;
use common::deserializers::string_or_number::from_string_or_number;
use ethers::types::H160;
use serde::Deserialize;
use std::str::FromStr;

#[derive(Deserialize)]
pub struct AlchemyGetNftsResponse {
    #[serde(rename(deserialize = "ownedNfts"))]
    pub owned_nfts: Vec<AlchemyNftResponse>,
    #[serde(rename(deserialize = "pageKey"))]
    pub page_key: Option<String>,
}

#[derive(Deserialize)]
pub struct AlchemyNftResponse {
    pub contract: AlchemyContractResponse,
    pub balance: String,
    pub title: String,
    pub description: String,
    pub metadata: Option<AlchemyMetadataResponse>,
    #[serde(rename(deserialize = "contractMetadata"))]
    pub contract_metadata: Option<AlchemyContractMetadataResponse>,
}

#[derive(Deserialize)]
pub struct AlchemyContractResponse {
    pub address: String,
}

#[derive(Deserialize)]
pub struct AlchemyMetadataResponse {
    pub image: Option<String>,
    pub attributes: Option<Vec<AlchemyMetadataAttributeResponse>>,
}

#[derive(Deserialize)]
pub struct AlchemyMetadataAttributeResponse {
    #[serde(deserialize_with = "from_string_or_number")]
    pub value: String,
    pub trait_type: String,
}

#[derive(Deserialize)]
pub struct AlchemyContractMetadataResponse {
    pub name: Option<String>,
    pub symbol: Option<String>,
}

impl TryFrom<AlchemyNftResponse> for NonFungibleTokenInfoDetail {
    type Error = anyhow::Error;

    fn try_from(value: AlchemyNftResponse) -> Result<Self, Self::Error> {
        let contract_address = H160::from_str(value.contract.address.as_str())
            .map_err(|e| anyhow!(e).context("Error parsing address"))?;

        let (name, symbol) = value
            .contract_metadata
            .map(|metadata| {
                (
                    metadata.name.unwrap_or_default(),
                    metadata.symbol.unwrap_or_default(),
                )
            })
            .unwrap_or_default();

        let (image, attributes) = match value.metadata {
            None => Default::default(),
            Some(AlchemyMetadataResponse { image, attributes }) => {
                let attrs = attributes
                    .into_iter()
                    .flatten()
                    .map(|attribute| NonFungibleTokenInfoAttribute {
                        value: attribute.value,
                        trait_type: attribute.trait_type,
                    })
                    .collect::<Vec<NonFungibleTokenInfoAttribute>>();
                (image.unwrap_or_default(), attrs)
            }
        };

        Ok(Self {
            contract_address,
            name,
            symbol,
            balance: value.balance,
            metadata: NonFungibleTokenInfoMetadata {
                name: value.title,
                description: value.description,
                image,
                attributes,
            },
        })
    }
}
