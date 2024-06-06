use serde::Deserialize;

#[derive(Deserialize, Debug)]
#[serde(rename_all(deserialize = "PascalCase"))]
pub struct DynamoDbStreamEvent {
    pub records: Vec<DynamoDbStreamEventData>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct DynamoDbStreamEventData {
    pub event_name: String,
    pub dynamodb: DynamoDbEvent,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all(deserialize = "PascalCase"))]
pub struct DynamoDbEvent {
    pub keys: serde_dynamo::Item,
    pub new_image: Option<serde_dynamo::Item>,
}
