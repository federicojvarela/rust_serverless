use chrono::Utc;
use ethers::types::Address;
use http::StatusCode;
use rstest::{fixture, rstest};
use serde::Deserialize;
use serde_json::{json, Value};
use std::str::FromStr;

use ana_tools::config_loader::ConfigLoader;

use model::sponsor_address_config::{SponsorAddressConfig, SponsorAddressConfigType};
use repositories::deserialize::deserialize_from_dynamo;
use repositories::sponsor_address_config::{
    SponsorAddressConfigDynamoDbResource, SponsorAddressConfigPk,
    SponsorAddressConfigRepositoryError,
};

use crate::fixtures::dynamodb::{dynamodb_fixture, DynamoDbFixture};
use crate::fixtures::lambda::{fixture, LambdaFixture};
use crate::helpers::dynamodb::{put_item, query_from_db, recreate_table};
use crate::helpers::lambda::LambdaResponse;

use crate::models::http_lambda_response::HttpLambdaEmptyResponse;
use common::test_tools::http::constants::{
    ADDRESS_FOR_MOCK_REQUESTS, ADDRESS_OF_RECIPIENT_FOR_MOCK_REQUESTS, CHAIN_ID_FOR_MOCK_REQUESTS,
    CLIENT_ID_FOR_MOCK_REQUESTS,
};

const FUNCTION_NAME: &str = "create_gas_pool";

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

    recreate_table(
        &dynamodb_fixture.dynamodb_client,
        SPONSOR_ADDRESS_CONFIG_TABLE_DEFINITION,
        table_name,
    )
    .await;

    LocalFixture { config }
}

fn build_request(body: String) -> Value {
    json!({
      "httpMethod": "POST",
      "headers": {
        "Content-Type": "application/json"
      },
      "pathParameters": {
        "chain_id": CHAIN_ID_FOR_MOCK_REQUESTS.to_string()
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
pub async fn gas_pool_creation_ok(
    fixture: &LambdaFixture,
    dynamodb_fixture: &DynamoDbFixture,
    #[future] local_fixture: LocalFixture,
) {
    let local_fixture = local_fixture.await;
    let table_name = &local_fixture.config.sponsor_address_config_table_name;

    let body = json!({
        "gas_pool_address": ADDRESS_FOR_MOCK_REQUESTS,
    })
    .to_string();
    let input = build_request(body);

    let response: Response = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|e| panic!("There was an error invoking {FUNCTION_NAME} {e:?}"));

    assert_eq!(StatusCode::CREATED, response.body.status_code);

    let key = SponsorAddressConfigPk::new(
        CLIENT_ID_FOR_MOCK_REQUESTS.to_owned(),
        CHAIN_ID_FOR_MOCK_REQUESTS,
        SponsorAddressConfigType::GasPool,
    );

    let key_condition_expression = "pk = :pk".to_owned();
    let result = query_from_db(
        &dynamodb_fixture.dynamodb_client,
        table_name,
        key_condition_expression,
        key,
    )
    .await
    .expect("Unable to get sponsor address item");

    let item: SponsorAddressConfig = deserialize_from_dynamo::<
        SponsorAddressConfigDynamoDbResource,
        SponsorAddressConfigRepositoryError,
    >(result)
    .unwrap()
    .try_into()
    .unwrap();

    let address = match item {
        SponsorAddressConfig::GasPool { address, .. } => Ok(address),
        _ => Err(()),
    }
    .unwrap();

    assert_eq!(
        Address::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap(),
        address
    );
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn gas_pool_creation_same_address_ok(
    fixture: &LambdaFixture,
    dynamodb_fixture: &DynamoDbFixture,
    #[future] local_fixture: LocalFixture,
) {
    let local_fixture = local_fixture.await;
    let table_name = &local_fixture.config.sponsor_address_config_table_name;

    let address_type = SponsorAddressConfigType::GasPool.as_str().to_owned();
    put_item(&dynamodb_fixture.dynamodb_client, table_name, &SponsorAddressConfigDynamoDbResource {
        pk: format!("CLIENT#{CLIENT_ID_FOR_MOCK_REQUESTS}#CHAIN_ID#{CHAIN_ID_FOR_MOCK_REQUESTS}#ADDRESS_TYPE#{address_type}"),
        sk: format!("ADDRESS#{}", ADDRESS_FOR_MOCK_REQUESTS),
        client_id: CLIENT_ID_FOR_MOCK_REQUESTS.to_owned(),
        chain_id: CHAIN_ID_FOR_MOCK_REQUESTS,
        address_type: SponsorAddressConfigType::GasPool.as_str().to_owned(),
        address: ADDRESS_FOR_MOCK_REQUESTS.to_owned(),
        forwarder_name: None,
        last_modified_at: Utc::now(),
    }).await;

    let body = json!({
        "gas_pool_address": ADDRESS_FOR_MOCK_REQUESTS,
    })
    .to_string();
    let input = build_request(body);

    let response: Response = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|e| panic!("There was an error invoking {FUNCTION_NAME} {e:?}"));

    assert_eq!(StatusCode::CREATED, response.body.status_code);
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn gas_pool_creation_already_exists(
    fixture: &LambdaFixture,
    dynamodb_fixture: &DynamoDbFixture,
    #[future] local_fixture: LocalFixture,
) {
    let local_fixture = local_fixture.await;
    let table_name = &local_fixture.config.sponsor_address_config_table_name;

    let address_type = SponsorAddressConfigType::GasPool.as_str().to_owned();
    put_item(&dynamodb_fixture.dynamodb_client, table_name, &SponsorAddressConfigDynamoDbResource {
        pk: format!("CLIENT#{CLIENT_ID_FOR_MOCK_REQUESTS}#CHAIN_ID#{CHAIN_ID_FOR_MOCK_REQUESTS}#ADDRESS_TYPE#{address_type}"),
        sk: format!("ADDRESS#{}", ADDRESS_FOR_MOCK_REQUESTS),
        client_id: CLIENT_ID_FOR_MOCK_REQUESTS.to_owned(),
        chain_id: CHAIN_ID_FOR_MOCK_REQUESTS,
        address_type: SponsorAddressConfigType::GasPool.as_str().to_owned(),
        address: ADDRESS_OF_RECIPIENT_FOR_MOCK_REQUESTS.to_owned(),
        forwarder_name: None,
        last_modified_at: Utc::now(),
    }).await;

    let body = json!({
        "gas_pool_address": ADDRESS_FOR_MOCK_REQUESTS,
    })
    .to_string();
    let input = build_request(body);

    let response: Response = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|e| panic!("There was an error invoking {FUNCTION_NAME} {e:?}"));

    assert_eq!(StatusCode::BAD_REQUEST, response.body.status_code);
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn gas_pool_creation_invalid_body(
    fixture: &LambdaFixture,
    _dynamodb_fixture: &DynamoDbFixture,
    #[future] _local_fixture: LocalFixture,
) {
    let body = json!({
        "address": ADDRESS_FOR_MOCK_REQUESTS,
    })
    .to_string();
    let input = build_request(body);

    let response: Response = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|e| panic!("There was an error invoking {FUNCTION_NAME} {e:?}"));

    assert_eq!(StatusCode::BAD_REQUEST, response.body.status_code);
}
