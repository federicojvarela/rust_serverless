use chrono::{DateTime, Utc};
use ethers::types::{H160, U256};
use repositories::sponsor_address_config::{
    SponsorAddressConfigDynamoDbResource, SponsorAddressConfigPk,
};
use rusoto_dynamodb::DynamoDbClient;
use serde_json::json;
use uuid::Uuid;

use common::test_tools::http::constants::{
    ADDRESS_FOR_MOCK_REQUESTS, CLIENT_ID_FOR_MOCK_REQUESTS, HASH_FOR_MOCK_REQUESTS,
    PUBLIC_KEY_FOR_MOCK_REQUESTS,
};
use model::key::Key;
use model::nonce::Nonce;
use model::order::helpers::{signature_data, sponsored_data};
use model::order::{GenericOrderData, OrderState, OrderStatus, OrderType, SharedOrderData};
use model::sponsor_address_config::{SponsorAddressConfig, SponsorAddressConfigType};

use crate::helpers::dynamodb::put_item;

fn build_generic_order(
    order_id: Uuid,
    order_type: OrderType,
    transaction_hash: Option<String>,
    order_state: OrderState,
    client_id: String,
    last_modified_at: DateTime<Utc>,
    created_at: DateTime<Utc>,
) -> OrderStatus {
    let data = if order_type == OrderType::Sponsored {
        sponsored_data()
    } else {
        signature_data()
    };

    OrderStatus {
        order_id,
        order_version: "1".to_string(),
        order_type,
        transaction_hash,
        state: order_state,
        data: GenericOrderData {
            shared_data: SharedOrderData { client_id },
            data,
        },
        replaces: None,
        replaced_by: None,
        last_modified_at,
        created_at,
        error: None,
        policy: None,
        cancellation_requested: None,
    }
}

pub fn build_order(
    order_state: OrderState,
    order_type: OrderType,
    last_modified_at: DateTime<Utc>,
) -> OrderStatus {
    build_generic_order(
        Uuid::new_v4(),
        order_type,
        Some(HASH_FOR_MOCK_REQUESTS.to_string()),
        order_state,
        CLIENT_ID_FOR_MOCK_REQUESTS.to_string(),
        last_modified_at,
        Utc::now(),
    )
}

pub async fn put_key_creation_order(
    client: &DynamoDbClient,
    table_name: &str,
    order_id: Uuid,
    state: OrderState,
) -> OrderStatus {
    let order = OrderStatus {
        order_id,
        order_version: "1".to_string(),
        order_type: OrderType::KeyCreation,
        state,
        transaction_hash: None,
        data: GenericOrderData {
            shared_data: SharedOrderData {
                client_id: CLIENT_ID_FOR_MOCK_REQUESTS.to_string(),
            },
            data: json!({
                "address": "0xee9f75c5e249000ad06372c4f84fcad68eab17ca"
            }),
        },
        created_at: Utc::now(),
        last_modified_at: Utc::now(),
        replaced_by: None,
        replaces: None,
        error: None,
        policy: None,
        cancellation_requested: None,
    };

    put_item(client, table_name, &order).await;

    order
}

pub async fn put_key(
    client: &DynamoDbClient,
    table_name: &str,
    key_id: Uuid,
    address: String,
) -> Key {
    let key = Key {
        key_id,
        order_type: "KEY_CREATION_ORDER".to_string(),
        order_version: "1".to_string(),
        client_id: CLIENT_ID_FOR_MOCK_REQUESTS.to_string(),
        client_user_id: Uuid::new_v4().to_string(),
        address,
        owning_user_id: Uuid::new_v4(),
        public_key: PUBLIC_KEY_FOR_MOCK_REQUESTS.to_string(),
        created_at: Utc::now(),
    };
    put_item(client, table_name, &key).await;
    key
}

pub async fn put_nonce(
    client: &DynamoDbClient,
    table_name: &str,
    address: H160,
    chain_id: u64,
    nonce: U256,
) -> Nonce {
    let nonce = Nonce {
        address,
        chain_id,
        nonce,
        created_at: Utc::now(),
        last_modified_at: Utc::now(),
    };

    put_item(client, table_name, &nonce).await;
    nonce
}

pub async fn put_sponsor_address_config(
    client: &DynamoDbClient,
    table_name: &str,
    chain_id: u64,
    address_type: SponsorAddressConfigType,
) -> SponsorAddressConfig {
    let key = SponsorAddressConfigPk::new(
        CLIENT_ID_FOR_MOCK_REQUESTS.to_owned(),
        chain_id,
        address_type.clone(),
    );
    let forwarder_name = match address_type {
        SponsorAddressConfigType::GasPool => None,
        SponsorAddressConfigType::Forwarder => Some("Forwarder 1".to_owned()),
    };
    let item = SponsorAddressConfigDynamoDbResource {
        pk: key.pk,
        sk: ADDRESS_FOR_MOCK_REQUESTS.to_owned(),
        client_id: CLIENT_ID_FOR_MOCK_REQUESTS.to_owned(),
        chain_id,
        address_type: address_type.as_str().to_owned(),
        address: ADDRESS_FOR_MOCK_REQUESTS.to_owned(),
        forwarder_name,
        last_modified_at: Utc::now(),
    };
    put_item(client, table_name, &item).await;
    item.try_into().unwrap()
}
