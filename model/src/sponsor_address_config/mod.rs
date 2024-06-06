use anyhow::{anyhow, Error};
use common::deserializers::h160::h160;
use ethers::types::Address;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Deserialize, Debug, Serialize, Clone)]
pub enum SponsorAddressConfig {
    GasPool {
        client_id: String,
        chain_id: u64,
        #[serde(deserialize_with = "h160")]
        address: Address,
    },
    Forwarder {
        client_id: String,
        chain_id: u64,
        #[serde(deserialize_with = "h160")]
        address: Address,
        forwarder_name: String,
    },
}

impl SponsorAddressConfig {
    pub fn extract_address(&self) -> Result<Address, Error> {
        match self {
            SponsorAddressConfig::GasPool { address, .. } => Ok(*address),
            SponsorAddressConfig::Forwarder { address, .. } => Ok(*address),
        }
    }

    pub fn extract_forwarder_name(&self) -> Result<String, Error> {
        match self {
            SponsorAddressConfig::Forwarder { forwarder_name, .. } => Ok(forwarder_name.to_owned()),
            SponsorAddressConfig::GasPool { .. } => Err(anyhow!(
                "Tried extracting forwarder name but address was of type Gas Pool"
            )),
        }
    }
}

#[derive(Deserialize, Debug, Serialize, Clone, PartialEq)]
pub enum SponsorAddressConfigType {
    GasPool,
    Forwarder,
}

impl SponsorAddressConfigType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::GasPool => "GAS_POOL",
            Self::Forwarder => "FORWARDER",
        }
    }
}

impl FromStr for SponsorAddressConfigType {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "GAS_POOL" => Ok(Self::GasPool),
            "FORWARDER" => Ok(Self::Forwarder),
            other => Err(anyhow!(
                "Not supported SponsorAddressConfigType variant: {other}"
            )),
        }
    }
}
