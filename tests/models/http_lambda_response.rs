use common::deserializers::json_from_string::deserialize_json_string;
use serde::de::DeserializeOwned;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct HttpLambdaResponse<T: DeserializeOwned> {
    #[serde(rename(deserialize = "statusCode"))]
    pub status_code: u16,

    #[serde(deserialize_with = "deserialize_json_string")]
    pub body: T,
}

#[derive(Deserialize, Debug)]
pub struct HttpLambdaEmptyResponse {
    #[serde(rename(deserialize = "statusCode"))]
    pub status_code: u16,
}

#[derive(Deserialize, Debug)]
pub struct LambdaErrorResponse {
    pub code: String,
    pub message: String,
}
