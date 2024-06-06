use crate::tools::fixtures::e2e::*;
use crate::tools::helper::*;
use crate::tools::runner::run_e2e_test;
use reqwest::StatusCode;
use rstest::rstest;
use serde_json::json;

// ⚠️⚠️⚠️ println! should be replaced with report library in the future ⚠️⚠️⚠️

#[rstest]
#[tokio::test(flavor = "multi_thread")]
async fn e2e_test_sign_sponsored_with_no_auth(e2e_fixture: &E2EFixture) {
    run_e2e_test(
        "No Auth",
        |test_context| async move {
            let config = test_context.config;
            let reqwest_client = test_context.client;

            print_with_time("[+] [Sign Sponsored] - Sign Sponsored transaction".to_string());

            let response = post_without_retry(
                &reqwest_client,
                &format!(
                    "{}/api/v1/keys/{}/sign/sponsored",
                    config.env_url, config.custodial_address
                ),
                "",
                json!({
                    "transaction": {
                        "to": "0x00",
                        "deadline": "0",
                        "value": "0",
                        "data": "0x00",
                        "chain_id": "1"
                    }
                })
                .to_string(),
                "application/json",
            )
            .await;

            print_with_time("[=] [Sign Sponsored] - Assert Response".to_string());
            assert_eq!(StatusCode::UNAUTHORIZED, response.status());
        },
        e2e_fixture,
    )
    .await;
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
async fn e2e_test_update_gas_pool_address_ok(e2e_fixture: &E2EFixture) {
    run_e2e_test(
        "Update gas pool address without previous address configured",
        |test_context| async move {
            let config = test_context.config;
            let reqwest_client = test_context.client;

            print_with_time(
                "[+] [Gas Pool Address Update] - Trying to update gas pool address".to_string(),
            );

            let chain_id = test_context.chain_id;

            ////////////////////////////////////////////////////////////////////////
            // Step 1 - [Authorize] - Obtain an Access Token
            // Call the authorize function to obtain a bearer token for authentication.
            let bearer_token = authorize(&config, &reqwest_client).await;

            ////////////////////////////////////////////////////////////////////////
            // Step 2 - [Create Key] - Creates a new key to sign transactions
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
            // Step 2 - [Create Key] - Creates a new key to sign transactions
            let order_id = create_key(&config, &reqwest_client, &bearer_token).await;

            ////////////////////////////////////////////////////////////////////////
            // Step 3 - [Get order status] - Validates Order Status
            // Call the get_order_status function to check the status of the created key order.
            let new_address = monitor_and_get_address_order_status(
                &config,
                &reqwest_client,
                &bearer_token,
                order_id,
                "KEY_CREATION_ORDER",
            )
            .await;

            let response = put_without_retry(
                &reqwest_client,
                &format!(
                    "{}/api/v1/gas_pool/chains/{}/addresses/{}",
                    config.env_url, chain_id, address
                ),
                &bearer_token,
                json!({
                    "gas_pool_address": new_address
                })
                .to_string(),
                "application/json",
            )
            .await;

            print_with_time("[=] [Gas Pool Address Update] - Assert Response".to_string());
            assert_eq!(StatusCode::OK, response.status());
        },
        e2e_fixture,
    )
    .await;
}
