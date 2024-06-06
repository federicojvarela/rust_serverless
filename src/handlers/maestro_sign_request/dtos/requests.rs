use anyhow::anyhow;
use common::deserializers::u256::unsigned_integer_256;
use ethers::types::U256;
use model::order::policy::Policy;
use mpc_signature_sm::dtos::requests::transaction_request::{
    TransactionRequest, TransactionRequestNoNonce,
};
use mpc_signature_sm::maestro::dtos::MaestroAuthorizingEntityLevel;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use uuid::Uuid;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SignatureRequest {
    pub transaction: TransactionRequestNoNonce,
    pub key_id: Uuid,
    #[serde(default, deserialize_with = "unsigned_integer_256")]
    pub replacement_nonce: U256,
    pub policy: Policy,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct MaestroSignatureRequest {
    pub key_id: Uuid,
    pub authorizing_data: Vec<MaestroSignatureAuthorizingEntityRequest>,
    pub transaction_payload: String,
    pub transaction_type: MaestroTransactionType,
    pub replacement_nonce: U256,
    pub policies: Vec<String>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct MaestroSignatureAuthorizingEntityRequest {
    pub metadata: String,
    pub metadata_signature: String,
    pub authorizing_entity: String,
    pub level: MaestroAuthorizingEntityLevel,
}

impl TryFrom<SignatureRequest> for MaestroSignatureRequest {
    type Error = anyhow::Error;
    fn try_from(incoming: SignatureRequest) -> Result<Self, Self::Error> {
        let zero_nonce_transaction = incoming
            .transaction
            .into_transaction_request_with_nonce(0.into());

        let transaction_payload = hex::encode(zero_nonce_transaction.as_rlp()?);

        let mut authorizing_data = Vec::with_capacity(incoming.policy.approvals.len());
        for approval in incoming.policy.approvals {
            let response = approval.response.ok_or_else(|| {
                anyhow!(
                    "Response was not present for approver named: {}",
                    approval.name
                )
            })?;
            let level = MaestroAuthorizingEntityLevel::from_str(&approval.level)?;

            authorizing_data.push(MaestroSignatureAuthorizingEntityRequest {
                metadata: response.metadata.clone(),
                metadata_signature: response.metadata_signature.clone(),
                authorizing_entity: approval.name.clone(),
                level,
            });
        }

        Ok(Self {
            key_id: incoming.key_id,
            authorizing_data,
            transaction_payload,
            transaction_type: (&zero_nonce_transaction).into(),
            replacement_nonce: incoming.replacement_nonce,
            policies: vec![incoming.policy.name],
        })
    }
}

#[derive(Deserialize, Debug, Serialize, PartialEq, Eq, Clone)]
pub enum MaestroTransactionType {
    EvmStandard,
    EvmEIP1559,
    EvmEIP712,
}

impl From<&TransactionRequest> for MaestroTransactionType {
    fn from(value: &TransactionRequest) -> Self {
        match value {
            TransactionRequest::Legacy { .. } => Self::EvmStandard,
            TransactionRequest::Eip1559 { .. } => Self::EvmEIP1559,
            TransactionRequest::Sponsored { .. } => Self::EvmEIP712,
        }
    }
}
