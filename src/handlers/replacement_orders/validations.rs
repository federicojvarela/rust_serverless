use http::Response;
use model::order::{OrderStatus, OrderTransaction, OrderType};
use mpc_signature_sm::http::errors::validation_error_response;

use crate::dtos::{ReplacementRequest, ReplacementRequestType};

pub fn validate_new_gas_values(
    original_order_transaction: &OrderTransaction,
    new_gas_values: &ReplacementRequest,
) -> Result<(), Response<String>> {
    match (original_order_transaction, &new_gas_values.transaction) {
        (
            OrderTransaction::Legacy {
                gas_price: original_gas_price,
                ..
            },
            ReplacementRequestType::Legacy {
                gas_price: new_gas_price,
            },
        ) => {
            if new_gas_price <= original_gas_price {
                return Err(validation_error_response(
                    format!(
                        "original gas price ({original_gas_price}) is higher than new gas price ({new_gas_price})",
                    ),
                    None,
                ));
            }
        }
        (
            OrderTransaction::Eip1559 {
                max_fee_per_gas: original_max_fee_per_gas,
                max_priority_fee_per_gas: original_max_priority_fee_per_gas,
                ..
            },
            ReplacementRequestType::Eip1559 {
                max_fee_per_gas: new_max_fee_per_gas,
                max_priority_fee_per_gas: new_max_priority_fee_per_gas,
            },
        ) => {
            if new_max_fee_per_gas <= original_max_fee_per_gas {
                return Err(validation_error_response(
                    format!(
                        "original max fee per gas ({original_max_fee_per_gas}) is higher than new max fee per gas ({new_max_fee_per_gas})",
                    ),
                    None,
                ));
            }

            if new_max_priority_fee_per_gas <= original_max_priority_fee_per_gas {
                return Err(validation_error_response(
                    format!(
                        "original max fee priority per gas ({original_max_priority_fee_per_gas}) is higher than new max priority fee per gas ({new_max_priority_fee_per_gas})",
                    ),
                    None,
                ));
            }
        }
        (OrderTransaction::Legacy { .. }, ReplacementRequestType::Eip1559 { .. }) => {
            return Err(validation_error_response(
                "can't perform this operation on a legacy transaction with an EIP-1559 transaction"
                    .to_string(),
                None,
            ));
        }
        (OrderTransaction::Eip1559 { .. }, ReplacementRequestType::Legacy { .. }) => {
            return Err(validation_error_response(
                "can't perform this operation on an EIP-1559 transaction with a legacy transaction"
                    .to_string(),
                None,
            ));
        }
        (OrderTransaction::Sponsored { .. }, _) => {
            return Err(validation_error_response(
                "sponsored transactions can't be sped up".to_string(),
                None,
            ))
        }
    };

    Ok(())
}

pub fn validate_order_type(original_order: &OrderStatus) -> Result<(), Response<String>> {
    if original_order.order_type != OrderType::Signature {
        return Err(validation_error_response(
            format!(
                "can't perform this operation for an order of type {}",
                original_order.order_type
            ),
            None,
        ));
    }

    Ok(())
}

#[cfg(test)]
pub mod tests {
    use chrono::Utc;
    use ethers::types::{Bytes, U256};
    use hex::FromHex;
    use http::{Response, StatusCode};
    use rstest::rstest;
    use serde_json::Value;
    use uuid::Uuid;

    use crate::ReplacementRequest;
    use common::test_tools::dtos::Error;
    use model::order::OrderState;
    use model::order::OrderStatus;
    use mpc_signature_sm::http::errors::{
        INCOMPATIBLE_ORDER_REPLACEMENT_ERROR_MESSAGE, VALIDATION_ERROR_CODE,
    };

    use common::test_tools::http::constants::{
        ADDRESS_OF_RECIPIENT_FOR_MOCK_REQUESTS, CHAIN_ID_FOR_MOCK_REQUESTS, VALUE_FOR_MOCK_REQUESTS,
    };
    use model::order::{GenericOrderData, OrderTransaction, SharedOrderData};

    use super::*;

    fn build_legacy_tx(gas_price: U256) -> OrderTransaction {
        OrderTransaction::Legacy {
            gas_price,
            chain_id: CHAIN_ID_FOR_MOCK_REQUESTS,
            gas: U256::from(1),
            to: ADDRESS_OF_RECIPIENT_FOR_MOCK_REQUESTS.to_string(),
            data: Bytes::from_hex(VALUE_FOR_MOCK_REQUESTS).unwrap(),
            value: VALUE_FOR_MOCK_REQUESTS.into(),
            nonce: None,
        }
    }

    pub fn build_legacy_request(gas_price: U256) -> ReplacementRequest {
        ReplacementRequest {
            transaction: ReplacementRequestType::Legacy { gas_price },
        }
    }

    fn build_eip_tx(max_fee_per_gas: U256, max_priority_fee_per_gas: U256) -> OrderTransaction {
        OrderTransaction::Eip1559 {
            chain_id: CHAIN_ID_FOR_MOCK_REQUESTS,
            gas: U256::from(1),
            to: ADDRESS_OF_RECIPIENT_FOR_MOCK_REQUESTS.to_string(),
            data: Bytes::from_hex(VALUE_FOR_MOCK_REQUESTS).unwrap(),
            value: VALUE_FOR_MOCK_REQUESTS.into(),
            max_fee_per_gas,
            max_priority_fee_per_gas,
            nonce: None,
        }
    }

    pub fn build_eip1559_request(
        max_fee_per_gas: U256,
        max_priority_fee_per_gas: U256,
    ) -> ReplacementRequest {
        ReplacementRequest {
            transaction: ReplacementRequestType::Eip1559 {
                max_fee_per_gas,
                max_priority_fee_per_gas,
            },
        }
    }

    pub fn check_incompatible_order_replacement_error(response: Response<String>) {
        assert_eq!(StatusCode::BAD_REQUEST, response.status());
        let body = serde_json::from_str::<Error>(response.body().as_str()).unwrap();
        assert_eq!(body.code, VALIDATION_ERROR_CODE);
        assert_eq!(
            body.message,
            INCOMPATIBLE_ORDER_REPLACEMENT_ERROR_MESSAGE.to_owned()
        );
    }

    fn assert_validation_error(result: Result<(), Response<String>>, message: &str) {
        assert!(result.is_err());
        let json: Value =
            serde_json::from_str(result.unwrap_err().body()).expect("JSON parsing failed");
        assert_eq!(json["code"], "validation");
        assert_eq!(json["message"], message);
    }

    fn build_order(
        state: OrderState,
        order_type: OrderType,
        replaced_by: Option<Uuid>,
    ) -> OrderStatus {
        OrderStatus {
            replaces: None,
            replaced_by,
            order_id: Uuid::new_v4(),
            order_type,
            data: GenericOrderData {
                shared_data: SharedOrderData {
                    client_id: "133".to_string(),
                },
                data: Value::Null,
            },
            created_at: Utc::now(),
            last_modified_at: Utc::now(),
            order_version: "1".to_string(),
            state,
            transaction_hash: None,
            error: None,
            policy: None,
            cancellation_requested: None,
        }
    }

    #[tokio::test]
    async fn legacy_original_legacy_speedup_higher_ok() {
        let original = build_legacy_tx(U256::from(1));
        let speedup = build_legacy_request(U256::from(2));
        let result = validate_new_gas_values(&original, &speedup);
        assert!(result.is_ok())
    }

    #[tokio::test]
    async fn eip_original_eip_speedup_higher_ok() {
        let original = build_eip_tx(U256::from(1), U256::from(1));
        let speedup = build_eip1559_request(U256::from(2), U256::from(2));
        let result = validate_new_gas_values(&original, &speedup);
        assert!(result.is_ok())
    }

    #[tokio::test]
    async fn legacy_original_legacy_speedup_lower_error() {
        let original = build_legacy_tx(U256::from(2));
        let speedup = build_legacy_request(U256::from(1));
        let result = validate_new_gas_values(&original, &speedup);
        assert_validation_error(
            result,
            "original gas price (2) is higher than new gas price (1)",
        );
    }

    #[tokio::test]
    async fn eip_original_eip_speedup_lower_error() {
        let original = build_eip_tx(U256::from(2), U256::from(2));
        let speedup = build_eip1559_request(U256::from(1), U256::from(1));
        let result = validate_new_gas_values(&original, &speedup);
        assert_validation_error(
            result,
            "original max fee per gas (2) is higher than new max fee per gas (1)",
        );
    }

    #[tokio::test]
    async fn eip_original_legacy_speedup_error() {
        let original = build_eip_tx(U256::from(2), U256::from(2));
        let speedup = build_legacy_request(U256::from(1));
        let result = validate_new_gas_values(&original, &speedup);
        assert_validation_error(
            result,
            "can't perform this operation on an EIP-1559 transaction with a legacy transaction",
        );
    }

    #[tokio::test]
    async fn legacy_original_eip_speedup_error() {
        let original = build_legacy_tx(U256::from(2));
        let speedup = build_eip1559_request(U256::from(1), U256::from(1));
        let result = validate_new_gas_values(&original, &speedup);
        assert_validation_error(
            result,
            "can't perform this operation on a legacy transaction with an EIP-1559 transaction",
        );
    }

    #[tokio::test]
    async fn order_valid_ok() {
        let order = build_order(OrderState::Submitted, OrderType::Signature, None);
        let result = validate_order_type(&order);
        assert!(result.is_ok())
    }

    #[rstest]
    #[case::completed(OrderType::KeyCreation)]
    #[case::received(OrderType::SpeedUp)]
    #[tokio::test]
    async fn order_not_valid_type_error(#[case] order_type: OrderType) {
        let order = build_order(OrderState::Submitted, order_type, None);
        let result = validate_order_type(&order);

        assert_validation_error(
            result,
            format!(
                "can't perform this operation for an order of type {}",
                order_type
            )
            .as_str(),
        );
    }
}
