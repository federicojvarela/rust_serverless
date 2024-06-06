use ana_tools::config_loader::ConfigLoader;
use chrono::{DateTime, Utc};
use ethers::types::U256;
use http::StatusCode;
use rstest::{fixture, rstest};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use common::test_tools::http::constants::{
    CONTRACT_ADDRESS_FOR_MOCK_FT_REQUESTS, HASH_FOR_MOCK_REQUESTS,
    MAX_FEE_PER_GAS_FOR_MOCK_REQUESTS, MAX_PRIORITY_FEE_PER_GAS_FOR_MOCK_REQUESTS,
};
use mpc_signature_sm::result::error::ErrorFromHttpHandler;

use crate::fixtures::dynamodb::{dynamodb_fixture, DynamoDbFixture};
use crate::fixtures::lambda::{fixture as lambda_fixture, LambdaFixture};
use crate::handlers::common_assertions::assert_error_from_http_handler;
use crate::helpers::dynamodb::{get_item_from_db, recreate_table};
use crate::helpers::lambda::LambdaResponse;
use crate::helpers::model::put_key;

const FUNCTION_NAME: &str = "mpc_nonce_writer";
const DEFAULT_ADDRESS: &str = "0x063edae5a0d8ebb5d24d1b84acd2b3115d4231b0";

// DEFAULT_U64_CHAIN_ID should match DEFAULT_HEX_STR_CHAIN_ID
const DEFAULT_U64_CHAIN_ID: &u64 = &1;
const DEFAULT_HEX_STR_CHAIN_ID: &str = "0x1";

// for cases where we need another hash in addition to HASH_FOR_MOCK_REQUESTS
const HASH_FOR_MOCK_REQUESTS_ALT: &str =
    "0x3a6ee8aaaa2d5d33e4137cb064c6069fc2eed5d6d3fb9df32b17849e0dae2664";

const TABLE_DEFINITION: &str = include_str!(
    "../../dockerfiles/integration-tests/localstack/dynamodb_tables/nonces_table.json"
);

const KEYS_TABLE_DEFINITION: &str =
    include_str!("../../dockerfiles/integration-tests/localstack/dynamodb_tables/keys_table.json");

#[derive(Deserialize)]
pub struct Config {
    pub nonces_table_name: String,
    pub keys_table_name: String,
}

pub struct LocalFixture {
    pub config: Config,
}

#[fixture]
async fn local_fixture(dynamodb_fixture: &DynamoDbFixture) -> LocalFixture {
    let config = ConfigLoader::load_test::<Config>();
    let table_name = config.nonces_table_name.clone();
    // Recreate the table to start fresh
    recreate_table(
        &dynamodb_fixture.dynamodb_client,
        TABLE_DEFINITION,
        table_name,
    )
    .await;

    recreate_table(
        &dynamodb_fixture.dynamodb_client,
        KEYS_TABLE_DEFINITION,
        config.keys_table_name.clone(),
    )
    .await;

    LocalFixture { config }
}

#[derive(Deserialize, Debug)]
struct NonceDynamoDbResource {
    pub address: String,
    pub chain_id: u64,
    pub nonce: U256,
    pub transaction_hash: String,
    pub created_at: DateTime<Utc>,
    pub last_modified_at: DateTime<Utc>,
}

#[derive(Serialize, Clone)]
struct NonceKeyDynamoDbResource<'a> {
    pub address: &'a str,
    pub chain_id: &'a u64,
}

fn get_eventbridge_event(
    address: Option<&str>,
    nonce: Option<&str>,
    hash: Option<&str>,
    chain_id: Option<&str>,
) -> Value {
    json!(
    {
        "version": "0",
        "id": "768820dd-d7cb-91c4-2d1b-3583f6cc7f5b",
        "detail-type": "EthereumTransaction",
        "source": "ana-chain-listener",
        "account": "572976003749",
        "time": "2023-04-28T16:48:03Z",
        "region": "us-west-2",
        "resources": [],
        "detail": {
            "hash": hash.unwrap_or(HASH_FOR_MOCK_REQUESTS),
            "nonce": nonce.unwrap_or("0x52c"),
            "blockHash": "0x862dc7e796ca0be9f7efae722c5963cb25cb419fef6f1a97195e4b2c96ae7b5a",
            "blockNumber": "0x105a036",
            "transactionIndex": "0xe",
            "from": address.unwrap_or(DEFAULT_ADDRESS),
            "to": CONTRACT_ADDRESS_FOR_MOCK_FT_REQUESTS,
            "value": "0x16345785d8a0000",
            "gasPrice": "0xa2063f7bd",
            "gas": "0x35538",
            "input": "0xb6f9de950000000000000000000000000000000000000000000407829e5b2d8f5341eb3a0000000000000000000000000000000000000000000000000000000000000080000000000000000000000000063edae5a0d8ebb5d24d1b84acd2b3115d4231b500000000000000000000000000000000000000000000000000000000644bf9340000000000000000000000000000000000000000000000000000000000000002000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc20000000000000000000000003407f39df63ca276fb1f22072b1fd06adde74ed5",
            "v": "0x1",
            "r": "0x72d5645d308f9915886a2c38d8924875eac13da45d729cc1a24a0480ce23f54",
            "s": "0x346eaf9a82d5290e60c49940c1a595e7d8f32729273069d5e492e14d1678a34f",
            "type": "0x2",
            "accessList": [],
            "maxPriorityFeePerGas": MAX_PRIORITY_FEE_PER_GAS_FOR_MOCK_REQUESTS,
            "maxFeePerGas": MAX_FEE_PER_GAS_FOR_MOCK_REQUESTS,
            "chainId": chain_id.unwrap_or(DEFAULT_HEX_STR_CHAIN_ID),
        }
    })
}

fn check_default_nonce_fields(
    nonce: &NonceDynamoDbResource,
    expected_nonce: u128,
    expected_hash: &str,
) {
    assert_eq!(DEFAULT_ADDRESS, nonce.address);
    assert_eq!(U256::from(expected_nonce), nonce.nonce);
    assert_eq!(expected_hash, nonce.transaction_hash);
    assert_eq!(*DEFAULT_U64_CHAIN_ID, nonce.chain_id);
}

#[rstest]
#[case::hex_nonce_and_chain_id(get_eventbridge_event(
    None,
    Some("0x1"),
    None,
    Some(DEFAULT_HEX_STR_CHAIN_ID)
))]
#[case::base10_nonce_and_chain_id(get_eventbridge_event(None, Some("1"), None, Some(&DEFAULT_U64_CHAIN_ID.to_string())))]
#[tokio::test(flavor = "multi_thread")]
pub async fn write_nonce_ok(
    lambda_fixture: &LambdaFixture,
    dynamodb_fixture: &DynamoDbFixture,
    #[future] local_fixture: LocalFixture,
    #[case] lambda_input: Value,
) {
    let local_fixture = local_fixture.await;

    let _ = put_key(
        &dynamodb_fixture.dynamodb_client,
        &local_fixture.config.keys_table_name,
        Uuid::new_v4(),
        DEFAULT_ADDRESS.to_owned(),
    )
    .await;

    let table_name = &local_fixture.config.nonces_table_name;
    let _: LambdaResponse<Value> = lambda_fixture
        .lambda
        .invoke(FUNCTION_NAME, &lambda_input)
        .await
        .unwrap_or_else(|e| panic!("There was an error invoking {FUNCTION_NAME}: {e:?}"));

    let key = NonceKeyDynamoDbResource {
        address: DEFAULT_ADDRESS,
        chain_id: DEFAULT_U64_CHAIN_ID,
    };

    let nonce: NonceDynamoDbResource =
        get_item_from_db(&dynamodb_fixture.dynamodb_client, table_name, key)
            .await
            .expect("Nonce not found");

    check_default_nonce_fields(&nonce, 2, HASH_FOR_MOCK_REQUESTS);
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
#[case::base10_nonce_and_chain_id(get_eventbridge_event(None, Some("1"), None, Some(&DEFAULT_U64_CHAIN_ID.to_string())))]
pub async fn overwrite_nonce_ok(
    lambda_fixture: &LambdaFixture,
    dynamodb_fixture: &DynamoDbFixture,
    #[future] local_fixture: LocalFixture,
    #[case] lambda_input: Value,
) {
    let local_fixture = local_fixture.await;

    let _ = put_key(
        &dynamodb_fixture.dynamodb_client,
        &local_fixture.config.keys_table_name,
        Uuid::new_v4(),
        DEFAULT_ADDRESS.to_owned(),
    )
    .await;

    let table_name = &local_fixture.config.nonces_table_name;
    let _: LambdaResponse<Value> = lambda_fixture
        .lambda
        .invoke(FUNCTION_NAME, &lambda_input)
        .await
        .unwrap_or_else(|e| panic!("There was an error invoking {FUNCTION_NAME}: {e:?}"));

    let key = NonceKeyDynamoDbResource {
        address: DEFAULT_ADDRESS,
        chain_id: DEFAULT_U64_CHAIN_ID,
    };

    let nonce: NonceDynamoDbResource = get_item_from_db(
        &dynamodb_fixture.dynamodb_client,
        table_name,
        key.to_owned(),
    )
    .await
    .expect("Nonce not found");

    check_default_nonce_fields(&nonce, 2, HASH_FOR_MOCK_REQUESTS);

    // re run lambda nonce writer to override the previous nonce with one more
    let input = get_eventbridge_event(
        Some(DEFAULT_ADDRESS),
        Some("0x3"),
        Some(HASH_FOR_MOCK_REQUESTS_ALT),
        Some(&DEFAULT_U64_CHAIN_ID.to_string()),
    );
    let _: LambdaResponse<Value> = lambda_fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|e| panic!("There was an error invoking {FUNCTION_NAME}: {e:?}"));

    let nonce: NonceDynamoDbResource = get_item_from_db(
        &dynamodb_fixture.dynamodb_client,
        table_name,
        key.to_owned(),
    )
    .await
    .expect("Nonce not found");

    check_default_nonce_fields(&nonce, 4, HASH_FOR_MOCK_REQUESTS_ALT);
    assert!(nonce.created_at < nonce.last_modified_at);
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn overwrite_nonce_with_the_same_value_ok(
    lambda_fixture: &LambdaFixture,
    dynamodb_fixture: &DynamoDbFixture,
    #[future] local_fixture: LocalFixture,
) {
    let local_fixture = local_fixture.await;

    let _ = put_key(
        &dynamodb_fixture.dynamodb_client,
        &local_fixture.config.keys_table_name,
        Uuid::new_v4(),
        DEFAULT_ADDRESS.to_owned(),
    )
    .await;

    let table_name = &local_fixture.config.nonces_table_name;
    let lambda_input = get_eventbridge_event(
        Some(DEFAULT_ADDRESS),
        Some("0x7"),
        Some(HASH_FOR_MOCK_REQUESTS),
        Some(&DEFAULT_U64_CHAIN_ID.to_string()),
    );

    let response: LambdaResponse<Value> = lambda_fixture
        .lambda
        .invoke(FUNCTION_NAME, &lambda_input)
        .await
        .unwrap_or_else(|e| panic!("There was an error invoking {FUNCTION_NAME}: {e:?}"));
    assert_eq!(StatusCode::OK, response.status);

    let key = NonceKeyDynamoDbResource {
        address: DEFAULT_ADDRESS,
        chain_id: DEFAULT_U64_CHAIN_ID,
    };

    let nonce: NonceDynamoDbResource = get_item_from_db(
        &dynamodb_fixture.dynamodb_client,
        table_name,
        key.to_owned(),
    )
    .await
    .expect("Nonce not found");

    check_default_nonce_fields(&nonce, 8, HASH_FOR_MOCK_REQUESTS);

    // re run lambda nonce writer with exact same input one more time - should cause no error
    let response: LambdaResponse<Value> = lambda_fixture
        .lambda
        .invoke(FUNCTION_NAME, lambda_input)
        .await
        .unwrap_or_else(|e| panic!("There was an error invoking {FUNCTION_NAME}: {e:?}"));
    assert_eq!(StatusCode::OK, response.status);

    let nonce: NonceDynamoDbResource = get_item_from_db(
        &dynamodb_fixture.dynamodb_client,
        table_name,
        key.to_owned(),
    )
    .await
    .expect("Nonce not found");

    check_default_nonce_fields(&nonce, 8, HASH_FOR_MOCK_REQUESTS);
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
#[case::base10_nonce_and_chain_id(get_eventbridge_event(None, Some("0x5"), None, Some(&DEFAULT_U64_CHAIN_ID.to_string())))]
pub async fn lower_value_nonce_should_not_overwrite_nonce(
    lambda_fixture: &LambdaFixture,
    dynamodb_fixture: &DynamoDbFixture,
    #[future] local_fixture: LocalFixture,
    #[case] lambda_input: Value,
) {
    let local_fixture = local_fixture.await;

    let _ = put_key(
        &dynamodb_fixture.dynamodb_client,
        &local_fixture.config.keys_table_name,
        Uuid::new_v4(),
        DEFAULT_ADDRESS.to_owned(),
    )
    .await;

    let table_name = &local_fixture.config.nonces_table_name;
    let response: LambdaResponse<Value> = lambda_fixture
        .lambda
        .invoke(FUNCTION_NAME, &lambda_input)
        .await
        .unwrap_or_else(|e| panic!("There was an error invoking {FUNCTION_NAME}: {e:?}"));
    assert_eq!(StatusCode::OK, response.status);

    let key = NonceKeyDynamoDbResource {
        address: DEFAULT_ADDRESS,
        chain_id: DEFAULT_U64_CHAIN_ID,
    };

    let nonce: NonceDynamoDbResource = get_item_from_db(
        &dynamodb_fixture.dynamodb_client,
        table_name,
        key.to_owned(),
    )
    .await
    .expect("Nonce not found");

    check_default_nonce_fields(&nonce, 6, HASH_FOR_MOCK_REQUESTS);

    // re run lambda nonce writer to override the previous nonce with a lower value - this should fail
    let input = get_eventbridge_event(
        Some(DEFAULT_ADDRESS),
        Some("0x2"),
        Some(HASH_FOR_MOCK_REQUESTS_ALT),
        Some("1"),
    );
    let response: LambdaResponse<Value> = lambda_fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|e| panic!("There was an error invoking {FUNCTION_NAME}: {e:?}"));

    assert_eq!(StatusCode::OK, response.status);

    let nonce: NonceDynamoDbResource = get_item_from_db(
        &dynamodb_fixture.dynamodb_client,
        table_name,
        key.to_owned(),
    )
    .await
    .expect("Nonce not found");

    check_default_nonce_fields(&nonce, 6, HASH_FOR_MOCK_REQUESTS);
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
#[case::base10_nonce_and_chain_id(get_eventbridge_event(None, Some("5"), None, Some(&DEFAULT_U64_CHAIN_ID.to_string())))]
pub async fn overwrite_nonce_wrong_chain_id_fail(
    lambda_fixture: &LambdaFixture,
    dynamodb_fixture: &DynamoDbFixture,
    #[future] local_fixture: LocalFixture,
    #[case] lambda_input: Value,
) {
    let local_fixture = local_fixture.await;

    let _ = put_key(
        &dynamodb_fixture.dynamodb_client,
        &local_fixture.config.keys_table_name,
        Uuid::new_v4(),
        DEFAULT_ADDRESS.to_owned(),
    )
    .await;

    let table_name = &local_fixture.config.nonces_table_name;
    let _: LambdaResponse<Value> = lambda_fixture
        .lambda
        .invoke(FUNCTION_NAME, &lambda_input)
        .await
        .unwrap_or_else(|e| panic!("There was an error invoking {FUNCTION_NAME}: {e:?}"));

    let key = NonceKeyDynamoDbResource {
        address: DEFAULT_ADDRESS,
        chain_id: DEFAULT_U64_CHAIN_ID,
    };

    let nonce: NonceDynamoDbResource = get_item_from_db(
        &dynamodb_fixture.dynamodb_client,
        table_name,
        key.to_owned(),
    )
    .await
    .expect("Nonce not found");

    check_default_nonce_fields(&nonce, 6, HASH_FOR_MOCK_REQUESTS);

    // re run lambda validate update with different chain_id
    // this should create a new DB item instead of update the one above with chain_id = 1
    let input = get_eventbridge_event(Some(DEFAULT_ADDRESS), Some("100"), None, Some("2"));
    let _: LambdaResponse<Value> = lambda_fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|e| panic!("There was an error invoking {FUNCTION_NAME}: {e:?}"));

    let nonce: NonceDynamoDbResource = get_item_from_db(
        &dynamodb_fixture.dynamodb_client,
        table_name,
        key.to_owned(),
    )
    .await
    .expect("Nonce not found");

    check_default_nonce_fields(&nonce, 6, HASH_FOR_MOCK_REQUESTS);
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn bad_request_invalid_nonce_error(
    lambda_fixture: &LambdaFixture,
    dynamodb_fixture: &DynamoDbFixture,
    #[future] local_fixture: LocalFixture,
) {
    let local_fixture = local_fixture.await;

    let _ = put_key(
        &dynamodb_fixture.dynamodb_client,
        &local_fixture.config.keys_table_name,
        Uuid::new_v4(),
        DEFAULT_ADDRESS.to_owned(),
    )
    .await;

    let key = NonceKeyDynamoDbResource {
        address: DEFAULT_ADDRESS,
        chain_id: DEFAULT_U64_CHAIN_ID,
    };
    let table_name = &local_fixture.config.nonces_table_name;
    // pass an invalid nonce
    let input = get_eventbridge_event(None, Some("invalid"), None, Some("0x1"));

    let response: LambdaResponse<ErrorFromHttpHandler> = lambda_fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|e| panic!("There was an error invoking {FUNCTION_NAME}: {e:?}"));

    let nonce: Option<NonceDynamoDbResource> =
        get_item_from_db(&dynamodb_fixture.dynamodb_client, table_name, key).await;

    assert_error_from_http_handler(response, "invalid hex character");
    assert!(nonce.is_none());
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn write_nonce_with_invalid_from_address_fail(lambda_fixture: &LambdaFixture) {
    let input = get_eventbridge_event(
        Some("wrong_address"),
        Some("1"),
        None,
        Some(&DEFAULT_U64_CHAIN_ID.to_string()),
    );
    let response: LambdaResponse<ErrorFromHttpHandler> = lambda_fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .expect("There was an error invoking {FUNCTION_NAME}: {e:?}");

    assert_error_from_http_handler(response, "Invalid H160");
}
