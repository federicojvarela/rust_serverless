use serde::{Serialize, Serializer};
use std::convert::TryFrom;

pub trait Fees {}

#[derive(Debug)]
pub struct LambdaResponse<T: Fees> {
    pub chain_id: u64,

    /// EIP-1559
    pub max_priority_fee_per_gas: T,

    /// EIP-1559
    pub max_fee_per_gas: T,

    /// Legacy (EIP-155)
    pub gas_price: T,
}

impl<T> TryFrom<LambdaResponse<T>> for String
where
    T: Serialize + Clone + Fees,
{
    type Error = serde_json::error::Error;

    fn try_from(value: LambdaResponse<T>) -> Result<Self, Self::Error> {
        serde_json::to_string(&value)
    }
}

impl<T> Serialize for LambdaResponse<T>
where
    T: Serialize + Clone + Fees,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        #[derive(Serialize)]
        struct Eip1559<T> {
            max_priority_fee_per_gas: T,
            max_fee_per_gas: T,
        }

        #[derive(Serialize)]
        struct Legacy<T> {
            gas_price: T,
        }

        #[derive(Serialize)]
        struct Helper<T> {
            chain_id: u64,
            eip1559: Eip1559<T>,
            legacy: Legacy<T>,
        }

        let helper = Helper {
            chain_id: self.chain_id,
            eip1559: Eip1559 {
                max_priority_fee_per_gas: self.max_priority_fee_per_gas.clone(),
                max_fee_per_gas: self.max_fee_per_gas.clone(),
            },
            legacy: Legacy {
                gas_price: self.gas_price.clone(),
            },
        };

        helper.serialize(serializer)
    }
}
