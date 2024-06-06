use anyhow::anyhow;
use model::order::policy::Policy;
use mpc_signature_sm::result::error::{OrchestrationError, UnknownOrchestrationError};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Debug)]
pub struct MaestroFetchPolicyRequest {
    pub policy_name: String,
    pub domain_name: String,
}

#[derive(Deserialize)]
pub struct MaestroGetPolicyResponse {
    pub serialized_policy: String,
    pub policy_name: String,
}

#[derive(Deserialize)]
pub struct MaestroPolicyInfo {
    pub domain_approvals: MaestroApprovers,
    pub policy_name: String,
    pub tenant_approvals: MaestroApprovers,
}

#[derive(Deserialize)]
pub struct MaestroApprovers {
    pub optional: Vec<String>,
    pub required: Vec<String>,
}

#[derive(Serialize)]
pub struct ProcessApproverResponse {
    pub policy: Policy,
}

#[derive(Deserialize, Debug, thiserror::Error)]
pub enum MaestroFetchPolicyError {
    #[error("Error converting decoded text from json to struct")]
    DecodeJsonError,
    #[error("Error decoding text")]
    InvalidBase64DecodeText,
    #[error("Could not convert json Vec<u8> to String")]
    InvalidJson,
}

impl From<MaestroFetchPolicyError> for OrchestrationError {
    fn from(value: MaestroFetchPolicyError) -> Self {
        OrchestrationError::Unknown(UnknownOrchestrationError::GenericError(anyhow!(
            value.to_string()
        )))
    }
}
