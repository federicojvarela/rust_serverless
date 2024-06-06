use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde::Deserializer;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct HttpLambdaResponseBody<R: DeserializeOwned> {
    #[serde(rename = "body", deserialize_with = "deserialize_inner_escaped_json")]
    pub http_body: R,
    #[serde(rename = "statusCode")]
    pub http_status_code: u16,
}

#[derive(Debug, Deserialize)]
pub struct OrderAcceptedBody {
    pub order_id: Uuid,
}

/// Serde will fail to deserialize an escaped json string directly into a struct, if we instead deserialize to a string first
/// and then user from_str, the from_str can handle the escaped json.
fn deserialize_inner_escaped_json<'de, D, R: DeserializeOwned>(
    deserializer: D,
) -> Result<R, D::Error>
where
    D: Deserializer<'de>,
{
    let buf = String::deserialize(deserializer)?;
    serde_json::from_str(&buf).map_err(serde::de::Error::custom)
}
