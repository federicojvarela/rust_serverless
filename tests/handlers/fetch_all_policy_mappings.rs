use crate::config::ConfigLoader;
use chrono::Utc;
use common::serializers::h160::h160_to_lowercase_hex_string;
use http::StatusCode;
use repositories::address_policy_registry::{
    AddressPolicyRegistryDynamoDbResource, AddressPolicyRegistryPk,
};
use rstest::{fixture, rstest};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::fixtures::dynamodb::{dynamodb_fixture, DynamoDbFixture};
use crate::fixtures::lambda::{fixture, LambdaFixture};
use crate::helpers::dynamodb::{put_item, recreate_table};
use crate::helpers::lambda::LambdaResponse;
use common::test_tools::http::constants::{
    ADDRESS_FOR_MOCK_REQUESTS, CHAIN_ID_FOR_MOCK_REQUESTS, CLIENT_ID_FOR_MOCK_REQUESTS,
};

use ethers::types::H160;
use std::str::FromStr;

const FUNCTION_NAME: &str = "fetch_all_policy_mappings";
const POLICY_NAME: &str = "some_policy";

#[derive(Deserialize)]
struct EmptyResponse;

type Response = LambdaResponse<Value>;

const ADDRESS_POLICY_REGISTRY_TABLE_DEFINITION: &str = include_str!(
    "../../dockerfiles/integration-tests/localstack/dynamodb_tables/address_policy_registry_table.json"
);

#[derive(Deserialize)]
pub struct Config {
    pub address_policy_registry_table_name: String,
}

pub struct LocalFixture {
    config: Config,
}

#[derive(Deserialize)]
#[serde(rename_all(deserialize = "lowercase"))]
pub enum AddressPathParam {
    Default,

    #[serde(untagged)]
    Address(ethers::types::Address),
}

#[fixture]
async fn local_fixture(dynamodb_fixture: &DynamoDbFixture) -> LocalFixture {
    let config = ConfigLoader::load_test::<Config>();
    let table_name = config.address_policy_registry_table_name.clone();
    // Recreate the table to start fresh
    recreate_table(
        &dynamodb_fixture.dynamodb_client,
        ADDRESS_POLICY_REGISTRY_TABLE_DEFINITION,
        table_name,
    )
    .await;

    LocalFixture { config }
}

fn build_request() -> Value {
    json!({
      "httpMethod": "POST",
      "headers": {
        "Content-Type": "application/json"
      },
      "requestContext": {
          "authorizer": {"claims": {"client_id": CLIENT_ID_FOR_MOCK_REQUESTS}},
          "httpMethod": "POST",
          "requestTimeEpoch": 1589522
      },
      "body": ""
    })
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn fetch_all_address_polices_ok(
    fixture: &LambdaFixture,
    dynamodb_fixture: &DynamoDbFixture,
    #[future] local_fixture: LocalFixture,
) {
    let local_fixture = local_fixture.await;
    let table_name = &local_fixture.config.address_policy_registry_table_name;

    let base_address = H160::from_str(ADDRESS_FOR_MOCK_REQUESTS).expect("Invalid address format");

    let mut addresses = Vec::new();
    for i in 0..3 {
        let mut address_bytes = base_address.0;
        address_bytes[19] = address_bytes[19].wrapping_add(i);
        addresses.push(H160(address_bytes));
    }

    for address_vec in addresses.iter() {
        let address_hex_string = h160_to_lowercase_hex_string(address_vec.to_owned());

        let address_to_query = Some(H160::from_str(&address_hex_string).unwrap());
        let address_to_save = Some(address_hex_string);

        let key = AddressPolicyRegistryPk::new(
            CLIENT_ID_FOR_MOCK_REQUESTS.to_owned(),
            CHAIN_ID_FOR_MOCK_REQUESTS,
            address_to_query,
        );

        // Put the policy for each address
        put_item(
            &dynamodb_fixture.dynamodb_client,
            table_name,
            &AddressPolicyRegistryDynamoDbResource {
                pk: key.pk.clone(),
                sk: key.sk.clone(),
                client_id: CLIENT_ID_FOR_MOCK_REQUESTS.to_owned(),
                chain_id: CHAIN_ID_FOR_MOCK_REQUESTS,
                address: address_to_save,
                policy: POLICY_NAME.to_owned(),
                created_at: Utc::now(),
            },
        )
        .await;
    }

    let input = build_request();

    let response: Response = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|e| panic!("There was an error invoking {FUNCTION_NAME} {e:?}"));

    assert_eq!(StatusCode::OK, response.status);

    let response_body: Value =
        serde_json::from_str(response.body.get("body").unwrap().as_str().unwrap()).unwrap();

    let chains = response_body["chains"]
        .as_array()
        .expect("Expected 'chains' to be an array");
    let addresses = chains
        .first()
        .expect("Expected at least one chain in 'chains'")
        .get("addresses")
        .expect("Expected 'addresses' field in chain")
        .as_array()
        .expect("Expected 'addresses' to be an array");

    let policies: Vec<&str> = addresses
        .iter()
        .map(|address| {
            address["policy"]
                .as_str()
                .expect("Expected 'policy' to be a string")
        })
        .collect();

    assert_eq!(policies.len(), 3, "Expected exactly 3 policies");
    for (i, policy) in policies.iter().enumerate() {
        assert_eq!(
            *policy, POLICY_NAME,
            "Policy at index {} does not match expected value",
            i
        );
    }

    for address in addresses.iter() {
        let address_type = address["type"]
            .as_str()
            .expect("Expected 'type' to be a string");
        assert_eq!(
            address_type, "ADDRESS_TO",
            "Expected type to be 'ADDRESS_TO'"
        );
    }
}
