use serde::Deserialize;

#[derive(Deserialize)]
pub struct UpdatePolicyMappingRequest {
    pub policy: String,
}
