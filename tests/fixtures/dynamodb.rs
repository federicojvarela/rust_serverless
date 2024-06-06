use common::aws_clients::dynamodb::get_dynamodb_client;
use rstest::fixture;
use rusoto_dynamodb::DynamoDbClient;

pub struct DynamoDbFixture {
    pub dynamodb_client: DynamoDbClient,
}

#[fixture]
#[once]
pub fn dynamodb_fixture() -> DynamoDbFixture {
    DynamoDbFixture {
        dynamodb_client: get_dynamodb_client(),
    }
}
