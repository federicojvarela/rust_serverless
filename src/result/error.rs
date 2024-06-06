//! OrchestrationError is used as the main error that all of our lambdas can fail with.
//! All other errors, such as AWS Steps errors, encountered during the execution of a state machine
//! are mapped to this error.

use crate::blockchain::providers::BlockchainProviderError;
use lambda_runtime::Error as LambdaRuntimeError;
use serde::{self, Deserialize};
use serde_json::Value;
use std::fmt;
use std::fmt::{Display, Formatter};
use std::string::String;
use validator::ValidationErrors;

pub type Result<T> = std::result::Result<T, OrchestrationError>;
pub type LambdaRuntimeResult = std::result::Result<(), LambdaRuntimeError>;

#[derive(Debug, thiserror::Error)]
pub enum UnknownOrchestrationError {
    #[error("{0}")]
    JsonValue(serde_json::Value),
    #[error("{0:?}")]
    GenericError(#[source] anyhow::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum OrchestrationError {
    Validation(String),
    NotFound(String),
    Unknown(#[source] UnknownOrchestrationError),
}

impl OrchestrationError {
    pub fn unknown<T: Into<serde_json::Value>>(value: T) -> Self {
        let value: serde_json::Value = value.into();
        match value {
            Value::Bool(b) => {
                Self::Unknown(UnknownOrchestrationError::GenericError(anyhow::anyhow!(b)))
            }
            Value::Number(n) => {
                Self::Unknown(UnknownOrchestrationError::GenericError(anyhow::anyhow!(n)))
            }
            Value::String(s) => {
                Self::Unknown(UnknownOrchestrationError::GenericError(anyhow::anyhow!(s)))
            }
            Value::Array(_) | Value::Object(_) | Value::Null => {
                Self::Unknown(UnknownOrchestrationError::JsonValue(value))
            }
        }
    }
}

impl Display for OrchestrationError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl From<reqwest::Error> for OrchestrationError {
    fn from(e: reqwest::Error) -> Self {
        Self::Unknown(UnknownOrchestrationError::GenericError(e.into()))
    }
}

impl From<anyhow::Error> for OrchestrationError {
    fn from(e: anyhow::Error) -> Self {
        Self::Unknown(UnknownOrchestrationError::GenericError(e))
    }
}

impl From<reqwest_middleware::Error> for OrchestrationError {
    fn from(e: reqwest_middleware::Error) -> Self {
        Self::Unknown(UnknownOrchestrationError::GenericError(e.into()))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum LambdaError {
    #[error("{0:#}")]
    Unknown(#[source] anyhow::Error),
    #[error("{0}")]
    NotFound(String),
}

impl From<BlockchainProviderError> for LambdaError {
    fn from(value: BlockchainProviderError) -> Self {
        match value {
            BlockchainProviderError::Unknown(e) => LambdaError::Unknown(e),
        }
    }
}

impl From<ValidationErrors> for OrchestrationError {
    fn from(e: ValidationErrors) -> Self {
        OrchestrationError::Validation(format!("{e:#}"))
    }
}

#[derive(Deserialize, Debug)]
pub struct ErrorFromHttpHandler {
    #[serde(rename(deserialize = "errorMessage"))]
    pub error_message: String,
    #[serde(rename(deserialize = "errorType"))]
    pub error_type: String,
}
