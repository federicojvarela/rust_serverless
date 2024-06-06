use common::deserializers::bytes::bytes;
use common::deserializers::h160::h160;
use common::deserializers::u256::unsigned_integer_256;
use ethers::abi::{InvalidOutputType, Token, Tokenizable};
use ethers::types::{Address, Bytes, U256};
use serde::{Deserialize, Serialize};
use std::convert::TryFrom;
use std::str::FromStr;

#[derive(Deserialize, Debug, Serialize, PartialEq, Eq, Clone)]
pub struct SponsoredTransaction {
    #[serde(default, deserialize_with = "h160")]
    pub from: Address,

    #[serde(default, deserialize_with = "h160")]
    pub to: Address,

    #[serde(deserialize_with = "unsigned_integer_256")]
    pub value: U256,

    #[serde(deserialize_with = "unsigned_integer_256")]
    pub gas: U256,

    pub deadline: String,

    #[serde(deserialize_with = "bytes")]
    pub data: Bytes,
}

#[derive(Debug, PartialEq, Eq)]
pub struct U48(u64);

impl TryFrom<String> for U48 {
    type Error = &'static str;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let v = u64::from_str(value.as_str()).unwrap();

        if v > 2_u64.pow(48) - 1 {
            Err("Value out of range for U48")
        } else {
            Ok(U48(v))
        }
    }
}

impl Tokenizable for U48 {
    fn from_token(token: Token) -> Result<Self, InvalidOutputType> {
        match token {
            Token::Uint(value) => {
                let value_u64 = value.low_u64().to_string();
                U48::try_from(value_u64)
                    .map_err(|_| InvalidOutputType("Failed to convert Uint to U48".to_string()))
            }
            _ => Err(InvalidOutputType("Expected Uint token".to_string())),
        }
    }

    fn into_token(self) -> Token {
        Token::Uint(self.0.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_u48_into_token() {
        let u48_value =
            U48::try_from(123_456_789_u64.to_string()).expect("Value should be within U48 range");
        let token = u48_value.into_token();

        match token {
            Token::Uint(value) => assert_eq!(value, U256::from(123_456_789_u64)),
            _ => panic!("Expected Token::Uint"),
        }
    }

    #[test]
    fn test_u48_from_u64() {
        let u48_result = U48::try_from(123_456_789_u64.to_string());

        assert!(u48_result.is_ok());
        assert_eq!(u48_result.unwrap(), U48(123_456_789_u64));
    }

    #[test]
    fn test_u48_from_u64_out_of_range() {
        let u48_result = U48::try_from(2_u64.pow(48).to_string());
        assert!(u48_result.is_err());
    }
}
