use serde::Deserialize;
use uuid::Uuid;

#[derive(Deserialize, Debug)]
pub struct Error {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct OrderAcceptedBody {
    pub order_id: Uuid,
}
