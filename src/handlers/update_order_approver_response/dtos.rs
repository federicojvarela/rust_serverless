use model::order::policy::{ApprovalResponse, Policy};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Debug)]
pub struct ProcessApproverRequest {
    pub fetched: Fetched,
    #[serde(flatten)]
    pub approval_response: ApprovalResponse,
}

#[derive(Deserialize, Debug)]
pub struct Fetched {
    pub order: FetchedOrder,
}

#[derive(Deserialize, Debug)]
pub struct FetchedOrder {
    pub policy: Policy,
}

#[derive(Serialize, Debug)]
pub struct ProcessApproverResponse {
    pub policy: Policy,
}
