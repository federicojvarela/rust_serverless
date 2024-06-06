use std::str::FromStr;

use chrono::Utc;
use http::StatusCode;

use ana_tools::config_loader::ConfigLoader;

use ethers::types::Address;
use mpc_signature_sm::maestro::config::MaestroConfig;
use repositories::address_policy_registry::{
    AddressPolicyRegistryDynamoDbResource, AddressPolicyRegistryPk,
};
use rstest::{fixture, rstest};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::fixtures::dynamodb::{dynamodb_fixture, DynamoDbFixture};
use crate::fixtures::lambda::{fixture, LambdaFixture};
use crate::fixtures::secrets::{secrets_manager_fixture, SecretsManagerFixture};
use crate::helpers::dynamodb::{get_item_from_db, put_item, recreate_table};
use crate::helpers::lambda::LambdaResponse;
use crate::helpers::secrets::recreate_string_secret;
use crate::models::http_lambda_response::HttpLambdaEmptyResponse;
use common::test_tools::http::constants::{
    ADDRESS_FOR_MOCK_REQUESTS, CHAIN_ID_FOR_MOCK_REQUESTS, CLIENT_ID_FOR_MOCK_REQUESTS,
};

const FUNCTION_NAME: &str = "update_policy_mapping";
const POLICY_NAME: &str = "some_policy";
const POLICY_UPDATE: &str = "some_policy_update";

#[derive(Deserialize)]
struct EmptyResponse;

type Response = LambdaResponse<HttpLambdaEmptyResponse>;

const ADDRESS_POLICY_REGISTRY_TABLE_DEFINITION: &str = include_str!(
    "../../dockerfiles/integration-tests/localstack/dynamodb_tables/address_policy_registry_table.json"
);

#[derive(Deserialize)]
pub struct Config {
    pub address_policy_registry_table_name: String,
}

pub struct LocalFixture {
    pub config: Config,
}

#[fixture]
async fn local_fixture(
    dynamodb_fixture: &DynamoDbFixture,
    secrets_manager_fixture: &SecretsManagerFixture,
) -> LocalFixture {
    let config = ConfigLoader::load_test::<Config>();
    let maestro_config = ConfigLoader::load_test::<MaestroConfig>();

    let table_name = config.address_policy_registry_table_name.clone();
    // Recreate the table to start fresh
    recreate_table(
        &dynamodb_fixture.dynamodb_client,
        ADDRESS_POLICY_REGISTRY_TABLE_DEFINITION,
        table_name,
    )
    .await;

    // Recreate maestro api key secret
    recreate_string_secret(
        &secrets_manager_fixture.secrets_manager,
        &maestro_config.maestro_api_key_secret_name,
        "secret-value",
    )
    .await;

    LocalFixture { config }
}

fn build_request() -> Value {
    let body = json!({
        "policy": POLICY_UPDATE,
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
pub async fn address_policy_update_ok(
    fixture: &LambdaFixture,
    dynamodb_fixture: &DynamoDbFixture,
    #[future] local_fixture: LocalFixture,
) {
    let local_fixture = local_fixture.await;
    let table_name = &local_fixture.config.address_policy_registry_table_name;

    let key = AddressPolicyRegistryPk::new(
        CLIENT_ID_FOR_MOCK_REQUESTS.to_owned(),
        CHAIN_ID_FOR_MOCK_REQUESTS,
        Some(Address::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap()),
    );

    // Put the policy for CONTRACT_ADDRESS address
    put_item(
        &dynamodb_fixture.dynamodb_client,
        table_name,
        &AddressPolicyRegistryDynamoDbResource {
            pk: key.pk.clone(),
            sk: key.sk.clone(),
            client_id: CLIENT_ID_FOR_MOCK_REQUESTS.to_owned(),
            chain_id: CHAIN_ID_FOR_MOCK_REQUESTS,
            address: Some(ADDRESS_FOR_MOCK_REQUESTS.to_string()),
            policy: POLICY_NAME.to_owned(),
            created_at: Utc::now(),
        },
    )
    .await;

    let input = build_request();

    let response: Response = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|e| panic!("There was an error invoking {FUNCTION_NAME} {e:?}"));

    let item: AddressPolicyRegistryDynamoDbResource =
        get_item_from_db(&dynamodb_fixture.dynamodb_client, table_name, key)
            .await
            .expect("unable to get address policy item");

    assert_eq!(StatusCode::OK, response.body.status_code);
    assert_eq!(CLIENT_ID_FOR_MOCK_REQUESTS, item.client_id);
    assert_eq!(ADDRESS_FOR_MOCK_REQUESTS, item.address.unwrap());
    assert_eq!(POLICY_UPDATE, item.policy);
}
