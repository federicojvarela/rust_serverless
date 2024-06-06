use crate::tools::config::Config;
use common::block_on;
use rstest::fixture;
use rusoto_core::Region;
use rusoto_dynamodb::{AttributeValue, DynamoDb, DynamoDbClient, PutItemInput};
use std::collections::HashMap;
use uuid::Uuid;

fn build_key_pair(key: &str, value: String) -> (String, AttributeValue) {
    (
        key.to_owned(),
        AttributeValue {
            s: Some(value),
            ..AttributeValue::default()
        },
    )
}

async fn build_key_item(
    address: String,
    public_key: String,
    client_id: String,
) -> HashMap<String, AttributeValue> {
    let key_id = Uuid::new_v4().to_string();
    let mut map = HashMap::new();
    map.extend([
        build_key_pair("key_id", key_id),
        build_key_pair("address", address),
        build_key_pair("client_id", client_id),
        build_key_pair(
            "client_user_id",
            "xRFNiyiuWC5jH1tHA3pMFPLsFQl3SDUM".to_owned(),
        ),
        build_key_pair("created_at", "2023-08-02T14:44:06.815Z".to_owned()),
        build_key_pair("order_type", "KEY_CREATION_ORDER".to_owned()),
        build_key_pair("order_version", "1".to_owned()),
        build_key_pair("owning_user_id", Uuid::new_v4().to_string()),
        build_key_pair("public_key", public_key),
    ]);

    map
}

async fn put_key_item(
    dynamodb_client: &DynamoDbClient,
    item: HashMap<String, AttributeValue>,
    table_name: String,
) {
    dynamodb_client
        .put_item(PutItemInput {
            table_name,
            item,
            ..PutItemInput::default()
        })
        .await
        .unwrap();
}

pub struct BalanceFixture;

/// The wallet balance endpoints have an authorization layer that ensures that the address that it's
/// being queried is one that is handled by the wallet service. Ephemeral environments start with
/// an empty Keys table, so this validation will always fail. This fixture solves this problem by creating
/// an entry in the ephemeral Keys table for the funded address that it's read from the env files.
#[fixture]
#[once]
pub fn balance_fixture() -> BalanceFixture {
    block_on!(async {
        let config = Config::load_test();

        if !config.ephemeral {
            return;
        }

        let dynamodb_client = DynamoDbClient::new(Region::UsWest2);
        let table_name = format!("{}-keys", config.environment);
        let item = build_key_item(
            config.funded_address,
            config.funded_address_public_key,
            config.client_id,
        )
        .await;
        put_key_item(&dynamodb_client, item, table_name).await;
    });

    BalanceFixture
}
