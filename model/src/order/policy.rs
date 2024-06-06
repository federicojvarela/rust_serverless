use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Policy {
    pub name: String,
    pub approvals: Vec<Approval>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Approval {
    pub level: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response: Option<ApprovalResponse>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ApprovalResponse {
    pub order_id: String,
    pub status_reason: String,
    pub approval_status: i32,
    pub approver_name: String,
    pub metadata: String,
    pub metadata_signature: String,
}
