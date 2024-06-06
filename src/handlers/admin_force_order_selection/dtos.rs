use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use mpc_signature_sm::validations::uuid::validate_not_default_uuid;

#[derive(Clone, Debug, Deserialize, Validate)]
pub struct AdminForceOrderSelectionRequest {
    #[validate(custom = "validate_not_default_uuid")]
    pub order_id: Uuid,
}

#[derive(Serialize, Debug)]
pub struct AdminForceOrderSelectionResponse {}
