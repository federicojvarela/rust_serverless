use std::{fmt::Display, str::FromStr};

use crate::http::errors::validation_error_response;
use common::serializers::h160::h160_to_lowercase_hex_string;
use ethers::types::Address;
use http::Response;
use serde::Deserialize;

const DEFAULT_VALUE: &str = "default";

#[derive(Deserialize)]
#[serde(rename_all(deserialize = "lowercase"))]
pub enum AddressOrDefaultPathParam {
    Default,

    #[serde(untagged)]
    Address(ethers::types::Address),
}

impl AddressOrDefaultPathParam {
    /// Extracts the wrapped address if the structure contains one. If not returns None
    pub fn extract_address(self) -> Option<Address> {
        match self {
            AddressOrDefaultPathParam::Default => None,
            AddressOrDefaultPathParam::Address(a) => Some(a),
        }
    }
}

impl FromStr for AddressOrDefaultPathParam {
    type Err = Response<String>;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == DEFAULT_VALUE {
            Ok(Self::Default)
        } else {
            let address = Address::from_str(s).map_err(|e| {
                validation_error_response(
                    format!("there was an error parsing the url address: {e:?}"),
                    None,
                )
            })?;
            Ok(Self::Address(address))
        }
    }
}

impl Display for AddressOrDefaultPathParam {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AddressOrDefaultPathParam::Default => write!(f, "{DEFAULT_VALUE}"),
            AddressOrDefaultPathParam::Address(a) => {
                write!(f, "{}", h160_to_lowercase_hex_string(*a))
            }
        }
    }
}
