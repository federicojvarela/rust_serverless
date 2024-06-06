use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct LambdaErrorResponse {
    pub code: String,
    pub message: String,
}
