use crate::tools::fixtures::dynamodb::balance::{balance_fixture, BalanceFixture};
use crate::tools::fixtures::e2e::*;
use crate::tools::helper::*;
use crate::tools::runner::run_e2e_test;
use ethers::types::U256;
use reqwest::StatusCode;
use rstest::rstest;
use serde_json::json;

// ⚠️⚠️⚠️ println! should be replaced with report library in the future ⚠️⚠️⚠️

#[rstest]
#[tokio::test(flavor = "multi_thread")]
async fn e2e_test_address_with_ft_balance(
    e2e_fixture: &E2EFixture,
    _balance_fixture: &BalanceFixture,
) {
    run_e2e_test(
        "Address with balance",
        |test_context| async move {
            let config = test_context.config;
            let chain_id = test_context.chain_id;
            let reqwest_client = test_context.client;
            let ft_contract_address = config.get_network_by_chain_id(chain_id).ft_contract;
            ////////////////////////////////////////////////////////////////////////
            // Step 1 - [Authorize] - Obtain an Access Token
            // Call the authorize function to obtain a bearer token for authentication.
            let bearer_token = authorize(&config, &reqwest_client).await;

            let parsed_body_response = fungible_token_balance_request(
                &config,
                &reqwest_client,
                &bearer_token,
                chain_id,
                config.funded_address.clone(),
                vec![ft_contract_address.clone()],
            )
            .await;

            print_with_time("[=] [Fungible Token Balance] - Assert Response".to_string());

            let balance = parsed_body_response["data"][0]["balance"].as_str().unwrap();
            let contract_address = parsed_body_response["data"][0]["contract_address"]
                .as_str()
                .unwrap();
            assert!(!balance.is_empty());
            assert!(!U256::from_dec_str(balance).unwrap().is_zero());
            assert_eq!(contract_address, ft_contract_address);

            print_with_time(
                "[-] [Fungible Token Balance] - Get fungible token balance".to_string(),
            );
        },
        e2e_fixture,
    )
    .await;
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
async fn e2e_test_ft_balance_with_no_auth(
    e2e_fixture: &E2EFixture,
    _balance_fixture: &BalanceFixture,
) {
    run_e2e_test(
        "No Auth",
        |test_context| async move {
            let config = test_context.config;
            let chain_id = test_context.chain_id;
            let reqwest_client = test_context.client;
            let ft_contract_address = config.get_network_by_chain_id(chain_id).ft_contract;

            print_with_time(
                "[+] [Fungible Token Balance] - Get Fungible token balance".to_string(),
            );

            let response = post_without_retry(
                &reqwest_client,
                &format!(
                    "{}/api/v1/chains/{}/addresses/{}/tokens/ft/query",
                    config.env_url,
                    chain_id,
                    config.funded_address.clone()
                ),
                "",
                json!({ "contract_addresses": vec![ft_contract_address.clone()] }).to_string(),
                "application/json",
            )
            .await;

            print_with_time(
                "[-] [Fungible Token Balance] - Get Fungible token balance request".to_string(),
            );

            print_with_time("[=] [Fungible Token Balance] - Assert Response".to_string());

            assert_eq!(StatusCode::UNAUTHORIZED, response.status());
        },
        e2e_fixture,
    )
    .await;
}
