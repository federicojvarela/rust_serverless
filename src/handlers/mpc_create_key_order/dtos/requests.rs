use serde::Deserialize;
use validator::Validate;

#[derive(Deserialize, Validate)]
pub struct KeyRequestBody {
    #[validate(length(min = 1))]
    pub client_user_id: String,
}
