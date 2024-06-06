use crate::config::ConfigLoader;
use http::StatusCode;
use rstest::{fixture, rstest};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::fixtures::dynamodb::{dynamodb_fixture, DynamoDbFixture};
use crate::fixtures::lambda::{fixture, LambdaFixture};
use crate::helpers::dynamodb::recreate_table;
use crate::helpers::lambda::LambdaResponse;
use crate::models::http_lambda_response::HttpLambdaEmptyResponse;
use common::test_tools::http::constants::{
    ADDRESS_FOR_MOCK_REQUESTS, CHAIN_ID_FOR_MOCK_REQUESTS, CLIENT_ID_FOR_MOCK_REQUESTS,
};

const FUNCTION_NAME: &str = "update_gas_pool";

#[derive(Deserialize)]
struct EmptyResponse;

type Response = LambdaResponse<HttpLambdaEmptyResponse>;

const SPONSOR_ADDRESS_CONFIG_TABLE_DEFINITION: &str = include_str!(
    "../../../../dockerfiles/integration-tests/localstack/dynamodb_tables/sponsor_address_config_table.json"
);

#[derive(Deserialize)]
pub struct Config {
    pub sponsor_address_config_table_name: String,
}

pub struct LocalFixture {
    pub config: Config,
}

#[fixture]
async fn local_fixture(dynamodb_fixture: &DynamoDbFixture) -> LocalFixture {
    let config = ConfigLoader::load_test::<Config>();

    let table_name = config.sponsor_address_config_table_name.clone();
    // Recreate the table to start fresh
    recreate_table(
        &dynamodb_fixture.dynamodb_client,
        SPONSOR_ADDRESS_CONFIG_TABLE_DEFINITION,
        table_name,
    )
    .await;

    LocalFixture { config }
}

fn build_request() -> Value {
    let body = json!({
        "gas_pool_address": ADDRESS_FOR_MOCK_REQUESTS,
    })
    .to_string();

    json!({
      "httpMethod": "POST",
      "headers": {
        "Content-Type": "application/json"
      },
      "pathParameters": {
        "chain_id": CHAIN_ID_FOR_MOCK_REQUESTS.to_string(),
        "address": ADDRESS_FOR_MOCK_REQUESTS.to_string()
      },
      "requestContext": {
          "authorizer": {"claims": {"client_id": CLIENT_ID_FOR_MOCK_REQUESTS}},
          "httpMethod": "POST",
          "requestTimeEpoch": 1589522
      },
      "body": body
    })
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn gas_pool_update_ok(
    fixture: &LambdaFixture,
    _dynamodb_fixture: &DynamoDbFixture,
    #[future] local_fixture: LocalFixture,
) {
    let _local_fixture = local_fixture.await;

    let input = build_request();

    let response: Response = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|e| panic!("There was an error invoking {FUNCTION_NAME} {e:?}"));

    assert_eq!(StatusCode::OK, response.body.status_code);
}
