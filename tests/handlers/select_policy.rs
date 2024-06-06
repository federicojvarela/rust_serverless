use crate::config::ConfigLoader;
use crate::fixtures::dynamodb::{dynamodb_fixture, DynamoDbFixture};
use crate::fixtures::lambda::{fixture, LambdaFixture};
use crate::helpers::dynamodb::{put_item, recreate_table};
use crate::helpers::lambda::LambdaResponse;
use chrono::Utc;
use common::test_tools::http::constants::*;
use ethers::types::Address;
use std::str::FromStr;
use uuid::Uuid;

use repositories::address_policy_registry::{
    AddressPolicyRegistryDynamoDbResource, AddressPolicyRegistryPk,
};
use rstest::fixture;
use rstest::rstest;
use rusoto_dynamodb::DynamoDbClient;
use serde::Deserialize;
use serde_json::{json, Value};

const FUNCTION_NAME: &str = "select_policy";

const TABLE_DEFINITION: &str = include_str!(
    "../../dockerfiles/integration-tests/localstack/dynamodb_tables/address_policy_registry_table.json"
);

const CONTRACT_ADDRESS: &str = "0x000386e3f7559d9b6a2f5c46b4ad1a9587d59dc3";

#[derive(Deserialize)]
pub struct Config {
    pub address_policy_registry_table_name: String,
}

pub struct LocalFixture {
    pub dynamodb_client: DynamoDbClient,
    pub address_policy_registry_table_name: String,
}

fn get_input() -> Value {
    json!({
        "context": {
            "order_id": Uuid::new_v4(),
        },
        "payload": {
            "client_id": CLIENT_ID_FOR_MOCK_REQUESTS,
            "chain_id": CHAIN_ID_FOR_MOCK_REQUESTS,
            "address": CONTRACT_ADDRESS,
        }
    })
}

#[fixture]
async fn local_fixture(dynamodb_fixture: &DynamoDbFixture) -> LocalFixture {
    let config = ConfigLoader::load_test::<Config>();
    let table_name = config.address_policy_registry_table_name.as_str();
    // Recreate the table to start fresh
    recreate_table(
        &dynamodb_fixture.dynamodb_client,
        TABLE_DEFINITION,
        table_name.to_owned(),
    )
    .await;

    LocalFixture {
        dynamodb_client: dynamodb_fixture.dynamodb_client.clone(),
        address_policy_registry_table_name: config.address_policy_registry_table_name,
    }
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn specific_policy_ok(fixture: &LambdaFixture, #[future] local_fixture: LocalFixture) {
    let local_fixture = local_fixture.await;

    let specific_policy_name = "SpecificPolicy".to_owned();
    let default_policy_name = "DefaultPolicy".to_owned();

    let key = AddressPolicyRegistryPk::new(
        CLIENT_ID_FOR_MOCK_REQUESTS.to_owned(),
        CHAIN_ID_FOR_MOCK_REQUESTS,
        Some(Address::from_str(CONTRACT_ADDRESS).unwrap()),
    );

    // Put the policy for CONTRACT_ADDRESS address
    put_item(
        &local_fixture.dynamodb_client,
        &local_fixture.address_policy_registry_table_name,
        &AddressPolicyRegistryDynamoDbResource {
            pk: key.pk,
            sk: key.sk,
            client_id: CLIENT_ID_FOR_MOCK_REQUESTS.to_owned(),
            chain_id: CHAIN_ID_FOR_MOCK_REQUESTS,
            address: Some(CONTRACT_ADDRESS.to_owned()),
            policy: specific_policy_name.clone(),
            created_at: Utc::now(),
        },
    )
    .await;

    let key = AddressPolicyRegistryPk::new(
        CLIENT_ID_FOR_MOCK_REQUESTS.to_owned(),
        CHAIN_ID_FOR_MOCK_REQUESTS,
        None,
    );

    // Put the default policy
    put_item(
        &local_fixture.dynamodb_client,
        &local_fixture.address_policy_registry_table_name,
        &AddressPolicyRegistryDynamoDbResource {
            pk: key.pk,
            sk: key.sk,
            client_id: CLIENT_ID_FOR_MOCK_REQUESTS.to_owned(),
            chain_id: CHAIN_ID_FOR_MOCK_REQUESTS,
            address: None,
            policy: default_policy_name.clone(),
            created_at: Utc::now(),
        },
    )
    .await;

    let response: LambdaResponse<Value> = fixture
        .lambda
        .invoke(FUNCTION_NAME, get_input())
        .await
        .unwrap_or_else(|_| panic!("There was an error invoking {FUNCTION_NAME}"));

    assert_eq!(
        specific_policy_name,
        response.body["payload"]["policy_name"]
    );
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn default_policy_ok(fixture: &LambdaFixture, #[future] local_fixture: LocalFixture) {
    let local_fixture = local_fixture.await;

    let default_policy_name = "DefaultPolicy".to_owned();

    let key = AddressPolicyRegistryPk::new(
        CLIENT_ID_FOR_MOCK_REQUESTS.to_owned(),
        CHAIN_ID_FOR_MOCK_REQUESTS,
        None,
    );

    // Put the default policy
    put_item(
        &local_fixture.dynamodb_client,
        &local_fixture.address_policy_registry_table_name,
        &AddressPolicyRegistryDynamoDbResource {
            pk: key.pk,
            sk: key.sk,
            client_id: CLIENT_ID_FOR_MOCK_REQUESTS.to_owned(),
            chain_id: CHAIN_ID_FOR_MOCK_REQUESTS,
            address: None,
            policy: default_policy_name.clone(),
            created_at: Utc::now(),
        },
    )
    .await;

    let response: LambdaResponse<Value> = fixture
        .lambda
        .invoke(FUNCTION_NAME, get_input())
        .await
        .unwrap_or_else(|_| panic!("There was an error invoking {FUNCTION_NAME}"));

    assert_eq!(default_policy_name, response.body["payload"]["policy_name"]);
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn no_policy_ok(fixture: &LambdaFixture, #[future] local_fixture: LocalFixture) {
    let _ = local_fixture.await;

    let response: LambdaResponse<Value> = fixture
        .lambda
        .invoke(FUNCTION_NAME, get_input())
        .await
        .unwrap_or_else(|_| panic!("There was an error invoking {FUNCTION_NAME}"));

    assert!(
        response.body["errorMessage"]
            .as_str()
            .unwrap()
            .contains(
                &format!("there was no default policy configured for client {CLIENT_ID_FOR_MOCK_REQUESTS} and chain id {CHAIN_ID_FOR_MOCK_REQUESTS}")
            )
        )
}
