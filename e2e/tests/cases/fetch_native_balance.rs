use crate::tools::fixtures::dynamodb::balance::{balance_fixture, BalanceFixture};
use crate::tools::fixtures::e2e::*;
use crate::tools::helper::*;
use crate::tools::runner::run_e2e_test;
use ethers::types::U256;
use reqwest::StatusCode;
use rstest::rstest;

// ⚠️⚠️⚠️ println! should be replaced with report library in the future ⚠️⚠️⚠️

const ADDRESS: &str = "0x308044c83a7ac91e8e82ff34ccd760b5388c5729";
#[rstest]
#[tokio::test(flavor = "multi_thread")]
async fn e2e_test_empty_address(e2e_fixture: &E2EFixture) {
    run_e2e_test(
        "Empty Address",
        |test_context| async move {
            let config = test_context.config;
            let chain_id = test_context.chain_id;
            let reqwest_client = test_context.client;
            ////////////////////////////////////////////////////////////////////////
            // Step 1 - [Authorize] - Obtain an Access Token
            // Call the authorize function to obtain a bearer token for authentication.
            let bearer_token = authorize(&config, &reqwest_client).await;

            let parsed_body_response = native_token_balance_request(
                &config,
                &reqwest_client,
                &bearer_token,
                Some(chain_id),
                "".to_string(),
                false,
            )
            .await;

            print_with_time("[=] [Native Token Balance] - Assert Response".to_string());
            assert_eq!(parsed_body_response["code"], "validation");
            assert_eq!(
                parsed_body_response["message"],
                "address with wrong type in request path"
            );
            print_with_time("[-] [Native Token Balance] - Get Native token balance".to_string());
        },
        e2e_fixture,
    )
    .await;
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
async fn e2e_test_without_address_permission(e2e_fixture: &E2EFixture) {
    run_e2e_test(
        "Address without permission",
        |test_context| async move {
            let config = test_context.config;
            let chain_id = test_context.chain_id;
            let reqwest_client = test_context.client;
            ////////////////////////////////////////////////////////////////////////
            // Step 1 - [Authorize] - Obtain an Access Token
            // Call the authorize function to obtain a bearer token for authentication.
            let bearer_token = authorize(&config, &reqwest_client).await;

            let parsed_body_response = native_token_balance_request(
                &config,
                &reqwest_client,
                &bearer_token,
                Some(chain_id),
                ADDRESS.to_string(),
                false,
            )
            .await;

            print_with_time("[=] [Native Token Balance] - Assert Response".to_string());
            assert_eq!(parsed_body_response["code"], "unauthorized");
            assert_eq!(
                parsed_body_response["message"],
                "client is not authorized to make this call"
            );
            print_with_time("[-] [Native Token Balance] - Get Native token balance".to_string());
        },
        e2e_fixture,
    )
    .await;
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
async fn e2e_test_empty_chain_id(e2e_fixture: &E2EFixture) {
    let reqwest_client = &e2e_fixture.client;
    let config = &e2e_fixture.config;

    ////////////////////////////////////////////////////////////////////////
    // Step 1 - [Authorize] - Obtain an Access Token
    // Call the authorize function to obtain a bearer token for authentication.
    let bearer_token = authorize(config, reqwest_client).await;

    let parsed_body_response = native_token_balance_request(
        config,
        reqwest_client,
        &bearer_token,
        None,
        ADDRESS.to_string(),
        false,
    )
    .await;

    print_with_time("[=] [Native Token Balance] - Assert Response".to_string());
    assert_eq!(parsed_body_response["code"], "validation");
    assert_eq!(
        parsed_body_response["message"],
        "chain_id with wrong type in request path"
    );
    print_with_time("[-] [Native Token Balance] - Get Native token balance".to_string());
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
async fn e2e_test_address_with_balance(
    e2e_fixture: &E2EFixture,
    _balance_fixture: &BalanceFixture,
) {
    run_e2e_test(
        "Address with balance",
        |test_context| async move {
            let config = test_context.config;
            let chain_id = test_context.chain_id;
            let reqwest_client = test_context.client;
            ////////////////////////////////////////////////////////////////////////
            // Step 1 - [Authorize] - Obtain an Access Token
            // Call the authorize function to obtain a bearer token for authentication.
            let bearer_token = authorize(&config, &reqwest_client).await;

            let parsed_body_response = native_token_balance_request(
                &config,
                &reqwest_client,
                &bearer_token,
                Some(chain_id),
                config.funded_address.clone(),
                false,
            )
            .await;

            print_with_time("[=] [Native Token Balance] - Assert Response".to_string());

            let balance = parsed_body_response["balance"].as_str().unwrap();
            assert!(!balance.is_empty());
            assert!(!U256::from_dec_str(balance).unwrap().is_zero());

            print_with_time("[-] [Native Token Balance] - Get Native token balance".to_string());
        },
        e2e_fixture,
    )
    .await;
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
async fn e2e_test_fetch_native_balance_with_no_auth(e2e_fixture: &E2EFixture) {
    run_e2e_test(
        "No Auth",
        |test_context| async move {
            let config = test_context.config;
            let chain_id = test_context.chain_id;
            let reqwest_client = test_context.client;

            let response = post_without_retry(
                &reqwest_client,
                &format!(
                    "{}/api/v1/chains/{}/addresses/{}/tokens/native/query",
                    config.env_url, chain_id, ADDRESS
                ),
                "",
                "".to_string(),
                "application/json",
            )
            .await;

            print_with_time("[=] [Native Token Balance] - Assert Response".to_string());
            assert_eq!(StatusCode::UNAUTHORIZED, response.status());
        },
        e2e_fixture,
    )
    .await;
}
