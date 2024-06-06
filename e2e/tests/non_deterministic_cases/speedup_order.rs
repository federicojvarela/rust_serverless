use crate::tools::fixtures::e2e::*;
use crate::tools::helper::*;
use crate::tools::runner::run_e2e_test;
use ethers::types::U256;
use reqwest::StatusCode;
use rstest::rstest;
use uuid::Uuid;

// ⚠️⚠️⚠️ println! should be replaced with report library in the future ⚠️⚠️⚠️

#[rstest]
#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn e2e_test_cannot_speedup_wrong_state(e2e_fixture: &E2EFixture) {
    run_e2e_test(
        "order wrong state",
        |test_context| async move {
            let config = test_context.config;
            let chain_id = test_context.chain_id;
            let reqwest_client = test_context.client;
            ////////////////////////////////////////////////////////////////////////
            // Step 1 - [Authorize] - Obtain an Access Token
            // Call the authorize function to obtain a bearer token for authentication.
            let bearer_token = authorize(&config, &reqwest_client).await;

            ////////////////////////////////////////////////////////////////////////
            // Step 2 - [Create Key] - Creates a new key to sign transactions
            // Call the create_key function to generate a new key and receive its order ID.
            let order_id = create_key(&config, &reqwest_client, &bearer_token).await;

            ////////////////////////////////////////////////////////////////////////
            // Step 3 - [Get order status] - Validates Order Status
            // Call the get_order_status function to check the status of the created key order.
            let address = monitor_and_get_address_order_status(
                &config,
                &reqwest_client,
                &bearer_token,
                order_id,
                "KEY_CREATION_ORDER",
            )
            .await;

            ////////////////////////////////////////////////////////////////////////
            // Step 4 - [Get gas price prediction] - Validates Order Status
            // Call the get_gas_price_prediction function to check the status of the created key order.
            let historical_fees_response = get_gas_price_prediction(
                &config,
                &reqwest_client,
                &bearer_token,
                chain_id.to_string().as_str(),
                true,
            )
            .await;
            let (gas_price, _, _) = parse_high_fees(historical_fees_response);

            ////////////////////////////////////////////////////////////////////////
            // Step 4 - [Sign] - Signs a LEGACY transaction with the newly created key
            // Call the sign_legacy_transaction function to sign a LEGACY transaction.
            let legacy_transaction_order_id = sign_legacy_transaction(
                &config,
                &reqwest_client,
                &bearer_token,
                chain_id,
                address.clone(),
                gas_price,
            )
            .await;

            ////////////////////////////////////////////////////////////////////////
            // Step 5 - [Get order status] - Validates Order Status
            // Call the get_order_status function again to check the status of the LEGACY transaction order.
            let _ = monitor_and_get_order_status(
                &config,
                &reqwest_client,
                &bearer_token,
                legacy_transaction_order_id.clone(),
                "SIGNATURE_ORDER",
            )
            .await;

            ////////////////////////////////////////////////////////////////////////
            // Step 6 - [Cancel order] - Cancel the given order
            // Call the cancel function to validate wrong state.
            let response = request_speedup_legacy_transaction(
                &config,
                &reqwest_client,
                &bearer_token,
                legacy_transaction_order_id,
                gas_price,
                false,
            )
            .await;

            assert!(!response.status().is_success());
            print_with_time("[=] [Speedup Order] - Assert Speed up requested failed".to_string());
            let body_response =
                parse_body(response.text().await.expect("Failed to read response body"));
            assert_eq!(body_response["code"], "validation");
            assert_eq!(
                body_response["message"],
                "can't perform this operation for an order in state NOT_SUBMITTED"
            );
        },
        e2e_fixture,
    )
    .await;
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
async fn e2e_test_cannot_speedup_not_found(e2e_fixture: &E2EFixture) {
    run_e2e_test(
        "order not found",
        |test_context| async move {
            let config = test_context.config;
            let reqwest_client = test_context.client;
            ////////////////////////////////////////////////////////////////////////
            // Step 1 - [Authorize] - Obtain an Access Token
            // Call the authorize function to obtain a bearer token for authentication.
            let bearer_token = authorize(&config, &reqwest_client).await;

            ////////////////////////////////////////////////////////////////////////
            // Step 2 - [Speedup order] - Speed up the given order
            // Call the speed up function to validate not found.
            let order_id = Uuid::new_v4().to_string();
            let response = request_speedup_legacy_transaction(
                &config,
                &reqwest_client,
                &bearer_token,
                order_id.clone(),
                U256::from(90),
                false,
            )
            .await;

            assert!(!response.status().is_success());
            print_with_time("[=] [Speedup Order] - Assert Speed up requested failed".to_string());
            let body_response =
                parse_body(response.text().await.expect("Failed to read response body"));
            assert_eq!(body_response["code"], "order_not_found");
            assert_eq!(body_response["message"], order_id);
        },
        e2e_fixture,
    )
    .await;
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
async fn e2e_test_speedup_with_no_auth(e2e_fixture: &E2EFixture) {
    run_e2e_test(
        "No Auth",
        |test_context| async move {
            let config = test_context.config;
            let reqwest_client = test_context.client;

            let order_id = Uuid::new_v4().to_string();
            let response = request_speedup_legacy_transaction(
                &config,
                &reqwest_client,
                "",
                order_id.clone(),
                U256::from(90),
                false,
            )
            .await;

            print_with_time("[=] [Speedup Order] - Assert response".to_string());
            assert_eq!(StatusCode::UNAUTHORIZED, response.status());
        },
        e2e_fixture,
    )
    .await;
}
