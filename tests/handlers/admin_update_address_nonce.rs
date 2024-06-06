use crate::fixtures::dynamodb::{dynamodb_fixture, DynamoDbFixture};
use crate::fixtures::lambda::{fixture, LambdaFixture};
use crate::fixtures::provider::provider_fixture;
use crate::fixtures::provider::ProviderFixture;
use crate::helpers::chain::{get_transaction, send_transaction};
use crate::helpers::dynamodb::{get_item_from_db, put_item, recreate_table};
use crate::helpers::lambda::LambdaResponse;
use ana_tools::config_loader::ConfigLoader;
use chrono::{DateTime, Utc};
use common::aws_clients::dynamodb::get_dynamodb_client;
use ethers::providers::Middleware;
use ethers::types::{Address, H256, U256};
use hex::ToHex;
use http::StatusCode;

use repositories::nonces::nonces_repository_impl::NoncesRepositoryImpl;
use repositories::nonces::{NoncePrimaryKeyDynamoDbResource, NoncesRepository};
use rstest::*;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::str::FromStr;
use std::sync::Arc;

const FUNCTION_NAME: &str = "admin_update_address_nonce";
const TABLE_DEFINITION: &str = include_str!(
    "../../dockerfiles/integration-tests/localstack/dynamodb_tables/nonces_table.json"
);

// Ganache address and chain id
const ADDRESS: &str = "0x79118546Ac8Eb2E895aAE827Cf9ec239DD5439C7";
const ADDRESS_PK: &str = "0xd3451c75d4e764a197d9a0fef918763b858cd5aa228df299fd4627042429d29a";
const CHAIN_ID: u64 = 1337;

// Random address not used anyhwere
const ADDRESS_2: &str = "0x5fba5a9FCA7228Cb3426Fc2cb5c31dfCD0D1F3b8";

#[derive(Serialize, Deserialize)]
struct NonceDynamoDbResource {
    pub address: Address,
    pub chain_id: u64,
    pub nonce: U256,
    pub transaction_hash: String,
    pub created_at: DateTime<Utc>,
    pub last_modified_at: DateTime<Utc>,
}

#[derive(Deserialize)]
pub struct Config {
    pub nonces_table_name: String,
}

pub struct LocalFixture {
    pub config: Config,
    pub nonces_repository: Arc<dyn NoncesRepository>,
}

#[fixture]
async fn local_fixture(dynamodb_fixture: &DynamoDbFixture) -> LocalFixture {
    let config = ConfigLoader::load_test::<Config>();
    // Recreate the table to start fresh
    recreate_table(
        &dynamodb_fixture.dynamodb_client,
        TABLE_DEFINITION,
        config.nonces_table_name.clone(),
    )
    .await;

    let nonces_repository = Arc::new(NoncesRepositoryImpl::new(
        config.nonces_table_name.clone(),
        get_dynamodb_client(),
    )) as Arc<dyn NoncesRepository>;

    LocalFixture {
        config,
        nonces_repository,
    }
}

fn build_input(address: Address) -> Value {
    json!({ "address": address, "chain_id": CHAIN_ID })
}

fn build_dynamodb_nonce_entry(
    address: Address,
    chain_id: u64,
    nonce: U256,
) -> NonceDynamoDbResource {
    NonceDynamoDbResource {
        address,
        chain_id,
        nonce,
        transaction_hash: H256::random().encode_hex(),
        created_at: Utc::now(),
        last_modified_at: Utc::now(),
    }
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn admin_update_address_nonce_unused_address_must_be_0(
    dynamodb_fixture: &DynamoDbFixture,
    fixture: &LambdaFixture,
    #[future] local_fixture: LocalFixture,
) {
    let address = Address::from_str(ADDRESS_2).unwrap();
    let nonce = build_dynamodb_nonce_entry(address, CHAIN_ID, U256::from(4));
    let local_fixture = local_fixture.await;
    put_item(
        &dynamodb_fixture.dynamodb_client,
        &local_fixture.config.nonces_table_name,
        &nonce,
    )
    .await;

    let input = build_input(address);
    let response: LambdaResponse<Value> = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|e| panic!("There was an error invoking {FUNCTION_NAME}.{e:#?}"));

    let nonce_item: NonceDynamoDbResource = get_item_from_db(
        &dynamodb_fixture.dynamodb_client,
        &local_fixture.config.nonces_table_name,
        &NoncePrimaryKeyDynamoDbResource {
            address,
            chain_id: CHAIN_ID,
        },
    )
    .await
    .unwrap();

    assert_eq!(StatusCode::OK, response.status);
    let body = response.body;
    assert_eq!(body["old_nonce"].as_str().unwrap(), "0x4");
    assert_eq!(body["new_nonce"].as_str().unwrap(), "0x0");
    assert_eq!(U256::from(0), nonce_item.nonce);
    assert_eq!("", nonce_item.transaction_hash);
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn admin_update_address_nonce_ok(
    dynamodb_fixture: &DynamoDbFixture,
    fixture: &LambdaFixture,
    provider_fixture: &ProviderFixture,
    #[future] local_fixture: LocalFixture,
) {
    let address = Address::from_str(ADDRESS).unwrap();
    let nonce = build_dynamodb_nonce_entry(address, CHAIN_ID, U256::from(4));
    let local_fixture = local_fixture.await;
    put_item(
        &dynamodb_fixture.dynamodb_client,
        &local_fixture.config.nonces_table_name,
        &nonce,
    )
    .await;

    // Sine we can't (or don't know how to) reset ganache, we get the previous address nonce so the
    // tests pass even if the same address was previously used
    let previous_nonce = provider_fixture
        .provider
        .get_transaction_count(address, None)
        .await
        .unwrap();

    const TX_NUMBER: u8 = 5;
    for _ in 0..TX_NUMBER {
        let _ = send_transaction(
            &provider_fixture.provider,
            get_transaction(
                H256::from_str(ADDRESS_PK).unwrap(),
                Address::from_str(ADDRESS).unwrap(),
                Address::from_str(ADDRESS_2).unwrap(),
                CHAIN_ID,
            )
            .await,
        )
        .await;
    }

    let input = build_input(address);
    let response: LambdaResponse<Value> = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|e| panic!("There was an error invoking {FUNCTION_NAME}.{e:#?}"));

    let nonce_item: NonceDynamoDbResource = get_item_from_db(
        &dynamodb_fixture.dynamodb_client,
        &local_fixture.config.nonces_table_name,
        &NoncePrimaryKeyDynamoDbResource {
            address,
            chain_id: CHAIN_ID,
        },
    )
    .await
    .unwrap();

    assert_eq!(StatusCode::OK, response.status);
    let body = response.body;
    assert_eq!(body["old_nonce"].as_str().unwrap(), "0x4");
    assert_eq!(
        body["new_nonce"].as_str().unwrap(),
        format!("{:#0x}", U256::from(TX_NUMBER) + previous_nonce)
    );
    assert_eq!(U256::from(TX_NUMBER) + previous_nonce, nonce_item.nonce);
    assert_eq!("", nonce_item.transaction_hash);
}
