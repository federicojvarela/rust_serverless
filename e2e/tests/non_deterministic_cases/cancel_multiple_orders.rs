use crate::tools::fixtures::e2e::*;
use crate::tools::helper::*;
use crate::tools::runner::run_e2e_test;
use ethers::types::U256;
use reqwest::Response;
use rstest::rstest;

// ⚠️⚠️⚠️ println! should be replaced with report library in the future ⚠️⚠️⚠️

#[rstest]
#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn e2e_test_cancel_multiple_orders(e2e_fixture: &E2EFixture) {
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
            // Step 4 - [Sign] - Signs a LEGACY transaction with the newly created key
            // Call the sign_legacy_transaction function to sign a LEGACY transaction.

            let legacy_tx = || async {
                sign_legacy_transaction(
                    &config,
                    &reqwest_client,
                    &bearer_token,
                    chain_id,
                    address.clone(),
                    U256::from(90), // arbitrary value to ensure it doesn't get process
                )
                .await
            };

            let orders = tokio::join!(
                legacy_tx(),
                legacy_tx(),
                legacy_tx(),
                legacy_tx(),
                legacy_tx(),
                legacy_tx(),
                legacy_tx(),
                legacy_tx(),
                legacy_tx(),
                legacy_tx(),
            );

            ////////////////////////////////////////////////////////////////////////
            // Step 5 - [Cancel order] - Cancel the given order
            // Call the cancel function to validate wrong state.
            let request_cancel = |order_id: String| async {
                request_cancel_order_transaction(&config, &reqwest_client, &bearer_token, order_id)
                    .await
            };

            let cancel_responses = tokio::join!(
                request_cancel(orders.0.clone()),
                request_cancel(orders.1.clone()),
                request_cancel(orders.2.clone()),
                request_cancel(orders.3.clone()),
                request_cancel(orders.4.clone()),
                request_cancel(orders.5.clone()),
                request_cancel(orders.6.clone()),
                request_cancel(orders.7.clone()),
                request_cancel(orders.8.clone()),
                request_cancel(orders.9.clone()),
            );

            fn assert_cancelled_order(cancel_response: Response) {
                assert!(cancel_response.status().is_success());
                print_with_time(
                    "[=] [Cancel Order] - Assert Cancellation requested succeded".to_string(),
                );
            }

            assert_cancelled_order(cancel_responses.0);
            assert_cancelled_order(cancel_responses.1);
            assert_cancelled_order(cancel_responses.2);
            assert_cancelled_order(cancel_responses.3);
            assert_cancelled_order(cancel_responses.4);
            assert_cancelled_order(cancel_responses.5);
            assert_cancelled_order(cancel_responses.6);
            assert_cancelled_order(cancel_responses.7);
            assert_cancelled_order(cancel_responses.8);
            assert_cancelled_order(cancel_responses.9);

            ////////////////////////////////////////////////////////////////////////
            // Step 6 - [Get order status] - Validates Order Status
            // Call the get_order_status function again to check the status of the LEGACY transaction order.

            let monitor_status = |order_id: String| async {
                monitor_order_status_until_state(
                    &config,
                    &reqwest_client,
                    &bearer_token,
                    order_id,
                    "CANCELLED",
                )
                .await
            };

            let _ = tokio::join!(
                monitor_status(orders.0),
                monitor_status(orders.1),
                monitor_status(orders.2),
                monitor_status(orders.3),
                monitor_status(orders.4),
                monitor_status(orders.5),
                monitor_status(orders.6),
                monitor_status(orders.7),
                monitor_status(orders.8),
                monitor_status(orders.9),
            );
        },
        e2e_fixture,
    )
    .await;
}
