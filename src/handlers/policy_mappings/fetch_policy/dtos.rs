#[cfg(test)]
use serde::Deserialize;
use serde::Serialize;

#[derive(Serialize)]
#[cfg_attr(test, derive(Deserialize))]
pub struct FetchPolicyResponse {
    pub policy: String,
}
