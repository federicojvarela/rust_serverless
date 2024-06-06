use crate::result::error::LambdaError;

#[derive(Debug, thiserror::Error)]
pub enum AuthorizationProviderError {
    #[error("{0:#}")]
    Unknown(anyhow::Error),
}

impl From<AuthorizationProviderError> for LambdaError {
    fn from(value: AuthorizationProviderError) -> Self {
        match value {
            AuthorizationProviderError::Unknown(e) => Self::Unknown(e),
        }
    }
}
