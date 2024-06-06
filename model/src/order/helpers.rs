use std::str::FromStr;

use chrono::{DateTime, Utc};
use ethers::types::{H160, U256};
use serde_json::{json, Value};
use uuid::Uuid;

use common::test_tools::http::constants::{
    ADDRESS_FOR_MOCK_REQUESTS, ADDRESS_OF_RECIPIENT_FOR_MOCK_REQUESTS, CHAIN_ID_FOR_MOCK_REQUESTS,
    CLIENT_ID_FOR_MOCK_REQUESTS, GAS_FOR_MOCK_REQUESTS, GAS_PRICE_FOR_MOCK_REQUESTS,
    HASH_FOR_MOCK_REQUESTS, KEY_ID_FOR_MOCK_REQUESTS, MAX_FEE_PER_GAS_FOR_MOCK_REQUESTS,
    MAX_PRIORITY_FEE_PER_GAS_FOR_MOCK_REQUESTS, VALUE_FOR_MOCK_REQUESTS,
};

use crate::order::{
    GenericOrderData, OrderState, OrderStatus, OrderTransaction, OrderType, SharedOrderData,
    SignatureOrderData,
};

/// This file contains methods used ONLY in unit and integration testing.
pub fn signature_data() -> Value {
    signature_data_with_hash("")
}

pub fn signature_data_with_hash(txn_hash: &str) -> Value {
    json!(
    {
        "maestro_signature": "f87a80822710827530941d98bf1fe5ae430a98461bad3b872031767c9634824e2094640651",
        "address": ADDRESS_FOR_MOCK_REQUESTS,
        "key_id": KEY_ID_FOR_MOCK_REQUESTS,
        "transaction_hash": txn_hash,
        "transaction": {
            "chain_id": CHAIN_ID_FOR_MOCK_REQUESTS,
            "data": "0xef", // some random data
            "gas": GAS_FOR_MOCK_REQUESTS,
            "max_fee_per_gas": MAX_FEE_PER_GAS_FOR_MOCK_REQUESTS,
            "max_priority_fee_per_gas": MAX_PRIORITY_FEE_PER_GAS_FOR_MOCK_REQUESTS,
            "to": ADDRESS_OF_RECIPIENT_FOR_MOCK_REQUESTS,
            "value": VALUE_FOR_MOCK_REQUESTS
        }
    })
}

pub fn sponsored_typed_data() -> Value {
    json!(
        {
            "domain":{
                "chainId":"0xaa36a7",
                "name":"test",
                "verifyingContract":"0x0000000000000000000000000000000000000000",
                "version":"1"
            },
            "message":{
                "data":"0x00",
                "deadline": "1709643070",
                "from":"0x1c965d1241d0040a3fc2a030baeeefb35c155a4e",
                "gas":"75000",
                "nonce":"0",
                "to":ADDRESS_OF_RECIPIENT_FOR_MOCK_REQUESTS,
                "value": "0x1",
            },
            "primaryType":"ForwardRequest",
            "types":{
                "EIP712Domain":[
                {
                    "name":"name",
                    "type":"string"
                },
                {
                    "name":"version",
                    "type":"string"
                },
                {
                    "name":"chainId",
                    "type":"string"
                },
                {
                    "name":"verifyingContract",
                    "type":"address"
                }
                ],
                "ForwardRequest":[
                {
                    "name":"from",
                    "type":"address"
                },
                {
                    "name":"to",
                    "type":"address"
                },
                {
                    "name":"value",
                    "type":"string"
                },
                {
                    "name":"gas",
                    "type":"string"
                },
                {
                    "name":"nonce",
                    "type":"string"
                },
                {
                    "name":"deadline",
                    "type":"string"
                },
                {
                    "name":"data",
                    "type":"bytes"
                }
                ]
            }
        }
    )
}

pub fn sponsored_data() -> Value {
    json!(
    {
        "maestro_signature": "f87a80822710827530941d98bf1fe5ae430a98461bad3b872031767c9634824e2094640651",
        "address": ADDRESS_FOR_MOCK_REQUESTS,
        "key_id": KEY_ID_FOR_MOCK_REQUESTS,
        "transaction_hash": HASH_FOR_MOCK_REQUESTS,
        "transaction":{
            "chain_id":CHAIN_ID_FOR_MOCK_REQUESTS,
            "typed_data": sponsored_typed_data(),
            "to": ADDRESS_FOR_MOCK_REQUESTS,
            "sponsor_addresses": {
                "gas_pool_address": ADDRESS_FOR_MOCK_REQUESTS,
                "forwarder_address": ADDRESS_FOR_MOCK_REQUESTS,
                "forwarder_name": "Forwarder 1"
            }
        }
    })
}

pub fn cancellation_data() -> Value {
    order_data_with_hash("", "0x0")
}

pub fn speedup_data() -> Value {
    order_data_with_hash("", "0x1")
}

pub fn order_data_with_hash(txn_hash: &str, value: &str) -> Value {
    json!(
    {
        "maestro_signature": "e00a80822710827530941d98bf1fe5ae430a98461bad3b872031767c9634824e2094640651",
        "address": ADDRESS_FOR_MOCK_REQUESTS,
        "key_id": KEY_ID_FOR_MOCK_REQUESTS,
        "transaction_hash": txn_hash,
        "transaction": {
            "chain_id": CHAIN_ID_FOR_MOCK_REQUESTS,
            "data": "0x00",
            "gas": "0x55f1",
            "max_fee_per_gas": MAX_FEE_PER_GAS_FOR_MOCK_REQUESTS,
            "max_priority_fee_per_gas": MAX_PRIORITY_FEE_PER_GAS_FOR_MOCK_REQUESTS,
            "to": ADDRESS_OF_RECIPIENT_FOR_MOCK_REQUESTS,
            "value": value
        }
    })
}

pub fn signature_order_eip1559_data(value: U256) -> SignatureOrderData {
    SignatureOrderData {
        transaction: OrderTransaction::Eip1559 {
            to: ADDRESS_OF_RECIPIENT_FOR_MOCK_REQUESTS.into(),
            gas: GAS_FOR_MOCK_REQUESTS.into(),
            max_fee_per_gas: MAX_FEE_PER_GAS_FOR_MOCK_REQUESTS.into(),
            max_priority_fee_per_gas: MAX_PRIORITY_FEE_PER_GAS_FOR_MOCK_REQUESTS.into(),
            data: [].into(),
            chain_id: CHAIN_ID_FOR_MOCK_REQUESTS,
            value,
            nonce: None,
        },
        address: H160::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap(),
        key_id: Uuid::from_str(KEY_ID_FOR_MOCK_REQUESTS).unwrap(),
        maestro_signature: None,
    }
}

pub fn signature_order_legacy_data(value: U256) -> SignatureOrderData {
    SignatureOrderData {
        transaction: OrderTransaction::Legacy {
            to: ADDRESS_OF_RECIPIENT_FOR_MOCK_REQUESTS.into(),
            gas: GAS_FOR_MOCK_REQUESTS.into(),
            gas_price: GAS_PRICE_FOR_MOCK_REQUESTS.into(),
            data: [].into(),
            chain_id: CHAIN_ID_FOR_MOCK_REQUESTS,
            value,
            nonce: None,
        },
        address: H160::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap(),
        key_id: Uuid::from_str(KEY_ID_FOR_MOCK_REQUESTS).unwrap(),
        maestro_signature: None,
    }
}

// TODO
pub fn key_creation_data() -> Value {
    json!({})
}

pub fn build_signature_order(
    order_id: Uuid,
    order_state: OrderState,
    transaction_hash: Option<String>,
) -> OrderStatus {
    build_signature_order_with_client_id(
        order_id,
        order_state,
        transaction_hash,
        CLIENT_ID_FOR_MOCK_REQUESTS.to_string(),
    )
}

pub fn build_sponsored_order(order_id: Uuid, order_state: OrderState) -> OrderStatus {
    let mut generic_signature_order = build_generic_order(
        order_id,
        OrderType::Sponsored,
        CLIENT_ID_FOR_MOCK_REQUESTS.to_string(),
        None,
        None,
        None,
        None,
    );
    generic_signature_order.state = order_state;
    generic_signature_order
}

/// We usually use CLIENT_ID_FOR_MOCK_REQUESTS in tests, but in rare cases where we need
/// other value, we can call this method
pub fn build_signature_order_with_client_id(
    order_id: Uuid,
    order_state: OrderState,
    transaction_hash: Option<String>,
    client_id: String,
) -> OrderStatus {
    let mut generic_signature_order = build_generic_order(
        order_id,
        OrderType::Signature,
        client_id,
        None,
        None,
        None,
        None,
    );
    generic_signature_order.state = order_state;
    generic_signature_order.transaction_hash = transaction_hash;
    generic_signature_order
}

pub fn build_cancellation_order(
    order_id: Uuid,
    order_state: OrderState,
    replaces: Uuid,
    last_modified_at: Option<DateTime<Utc>>,
    transaction_hash: Option<String>,
) -> OrderStatus {
    let mut generic_speedup_order = build_generic_order(
        order_id,
        OrderType::Cancellation,
        CLIENT_ID_FOR_MOCK_REQUESTS.to_string(),
        None,
        Some(replaces),
        last_modified_at,
        transaction_hash,
    );
    generic_speedup_order.state = order_state;
    generic_speedup_order
}

pub fn build_speedup_order(
    order_id: Uuid,
    order_state: OrderState,
    replaces: Uuid,
    last_modified_at: DateTime<Utc>,
) -> OrderStatus {
    let mut generic_speedup_order = build_generic_order(
        order_id,
        OrderType::SpeedUp,
        CLIENT_ID_FOR_MOCK_REQUESTS.to_string(),
        None,
        Some(replaces),
        Some(last_modified_at),
        None,
    );
    generic_speedup_order.state = order_state;
    generic_speedup_order
}

/// Rust has a recommended limit of 7 args max for a method;
/// so we pass values for which there are no reasonable defaults or values that are hard to re-assign.
/// As we need to pass more args, we can consider setting replaced_by, replaces and last_modified_at
/// to None and then re-assigning them to the values we want after calling this method.
fn build_generic_order(
    order_id: Uuid,
    order_type: OrderType,
    client_id: String,
    replaced_by: Option<Uuid>,
    replaces: Option<Uuid>,
    last_modified_at: Option<DateTime<Utc>>,
    transaction_hash: Option<String>,
) -> OrderStatus {
    let data = match order_type {
        OrderType::Signature => signature_data(),
        OrderType::Sponsored => sponsored_data(),
        OrderType::SpeedUp => speedup_data(),
        OrderType::KeyCreation => key_creation_data(),
        OrderType::Cancellation => cancellation_data(),
    };

    OrderStatus {
        order_id,
        order_type,
        state: OrderState::Received,
        transaction_hash,
        order_version: "1".to_owned(),
        data: GenericOrderData {
            shared_data: SharedOrderData { client_id },
            data,
        },
        created_at: Utc::now(),
        last_modified_at: last_modified_at.unwrap_or(Utc::now()),
        replaced_by,
        replaces,
        error: None,
        policy: None,
        cancellation_requested: None,
    }
}
