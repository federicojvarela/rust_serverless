pub mod address_validator;

use crate::result::error::LambdaError;
use async_trait::async_trait;

#[derive(Debug, thiserror::Error)]
pub enum AddressValidatorError {
    #[error("{0:#}")]
    Unknown(anyhow::Error),
}

impl From<AddressValidatorError> for LambdaError {
    fn from(error: AddressValidatorError) -> Self {
        match error {
            AddressValidatorError::Unknown(e) => LambdaError::Unknown(e),
        }
    }
}
#[async_trait]
pub trait AddressValidator {
    async fn valid_from_address(&self, address: String) -> Result<bool, AddressValidatorError>;
}
