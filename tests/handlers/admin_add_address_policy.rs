use crate::config::ConfigLoader;
use common::serializers::h160::h160_to_lowercase_hex_string;
use ethers::types::Address;
use http::StatusCode;
use rstest::*;
use rusoto_dynamodb::DynamoDbClient;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::str::FromStr;
use std::sync::Arc;
use validator::Validate;

use common::aws_clients::dynamodb::get_dynamodb_client;
use common::test_tools::http::constants::{
    ADDRESS_FOR_MOCK_REQUESTS, CHAIN_ID_FOR_MOCK_REQUESTS, CLIENT_ID_FOR_MOCK_REQUESTS,
    INVALID_H160_MESSAGE,
};
use repositories::address_policy_registry::address_policy_registry_repository_impl::AddressPolicyRegistryRepositoryImpl;
use repositories::address_policy_registry::{
    AddressPolicyRegistryPk, AddressPolicyRegistryRepository,
};

use crate::fixtures::dynamodb::{dynamodb_fixture, DynamoDbFixture};
use crate::fixtures::lambda::{fixture, LambdaFixture};
use crate::helpers::dynamodb::{get_item_from_db, recreate_table};
use crate::helpers::lambda::LambdaResponse;

const FUNCTION_NAME: &str = "admin_add_address_policy";
const TABLE_DEFINITION: &str = include_str!(
    "../../dockerfiles/integration-tests/localstack/dynamodb_tables/address_policy_registry_table.json"
);
const POLICY_NAME: &str = "test-policy";

#[derive(Deserialize, Serialize)]
struct PolicyDynamoDbResource {
    pub policy: String,
}

#[derive(Deserialize)]
pub struct Config {
    pub address_policy_registry_table_name: String,
}

pub struct LocalFixture {
    pub config: Config,
    pub dynamodb_client: DynamoDbClient,
    pub address_policy_registry_repository: Arc<dyn AddressPolicyRegistryRepository>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Validate)]
pub struct AdminAddAddressPolicyRequest {
    pub address: Option<String>,
    pub chain_id: u64,
    pub client_id: String,
    pub policy: String,
}

#[fixture]
async fn local_fixture(dynamodb_fixture: &DynamoDbFixture) -> LocalFixture {
    let config = ConfigLoader::load_test::<Config>();

    let table_name = config.address_policy_registry_table_name.clone();
    // Recreate the table to start fresh
    recreate_table(
        &dynamodb_fixture.dynamodb_client,
        TABLE_DEFINITION,
        table_name,
    )
    .await;

    let address_policy_registry_repository = Arc::new(AddressPolicyRegistryRepositoryImpl::new(
        config.address_policy_registry_table_name.clone(),
        get_dynamodb_client(),
    )) as Arc<dyn AddressPolicyRegistryRepository>;

    LocalFixture {
        config,
        dynamodb_client: dynamodb_fixture.dynamodb_client.clone(),
        address_policy_registry_repository,
    }
}

fn build_add_policy_input(address: Option<String>, policy_name: String) -> Value {
    json!({
        "address": address,
        "chain_id": CHAIN_ID_FOR_MOCK_REQUESTS,
        "client_id": CLIENT_ID_FOR_MOCK_REQUESTS,
        "policy": policy_name
    })
}

#[rstest]
#[case::mock_address(Some(Address::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap()), "SpecificPolicy")]
#[case::default_address(None, "DefaultPolicy")]
#[tokio::test(flavor = "multi_thread")]
pub async fn admin_add_address_policy_ok(
    fixture: &LambdaFixture,
    #[future] local_fixture: LocalFixture,
    #[case] address: Option<Address>,
    #[case] policy_name: &str,
) {
    let local_fixture = local_fixture.await;
    let put_policy_input = build_add_policy_input(
        address.map(h160_to_lowercase_hex_string),
        policy_name.to_string(),
    );

    // store a new policy
    let add_policy_response: LambdaResponse<Value> = fixture
        .lambda
        .invoke(FUNCTION_NAME, put_policy_input)
        .await
        .unwrap_or_else(|_| panic!("There was an error invoking {FUNCTION_NAME}"));
    assert_eq!(StatusCode::OK, add_policy_response.status);

    // check policy was added
    let key = AddressPolicyRegistryPk::new(
        CLIENT_ID_FOR_MOCK_REQUESTS.to_owned(),
        CHAIN_ID_FOR_MOCK_REQUESTS,
        address,
    );

    let get_policy_response: PolicyDynamoDbResource = get_item_from_db(
        &local_fixture.dynamodb_client,
        &local_fixture.config.address_policy_registry_table_name,
        key,
    )
    .await
    .expect("Policy not found");

    assert_eq!(policy_name.to_string(), get_policy_response.policy);
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn admin_add_bad_address_policy_error(
    fixture: &LambdaFixture,
    #[future] local_fixture: LocalFixture,
) {
    let _local_fixture = local_fixture.await;

    // store a new policy
    let put_policy_input =
        build_add_policy_input(Some("bad_address".to_string()), POLICY_NAME.to_string());
    let add_policy_response: LambdaResponse<Value> = fixture
        .lambda
        .invoke(FUNCTION_NAME, put_policy_input)
        .await
        .unwrap_or_else(|_| panic!("There was an error invoking {FUNCTION_NAME}"));

    assert!(add_policy_response.body["errorMessage"]
        .to_string()
        .contains(INVALID_H160_MESSAGE));
    assert_eq!(
        StatusCode::INTERNAL_SERVER_ERROR,
        add_policy_response.status
    );
}
