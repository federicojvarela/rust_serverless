use crate::tools::fixtures::dynamodb::balance::{balance_fixture, BalanceFixture};
use crate::tools::fixtures::e2e::*;
use crate::tools::helper::*;
use crate::tools::runner::run_e2e_test;
use reqwest::StatusCode;
use rstest::rstest;
use serde_json::json;

// ⚠️⚠️⚠️ println! should be replaced with report library in the future ⚠️⚠️⚠️

#[rstest]
#[tokio::test(flavor = "multi_thread")]
async fn e2e_test_address_with_balance(
    e2e_fixture: &E2EFixture,
    _balance_fixture: &BalanceFixture,
) {
    run_e2e_test(
        "Query NFTs for address with balance",
        |test_context| async move {
            let config = test_context.config;
            let chain_id = test_context.chain_id;
            let reqwest_client = test_context.client;

            ////////////////////////////////////////////////////////////////////////
            // Step 1 - [Authorize] - Obtain an Access Token
            let bearer_token = authorize(&config, &reqwest_client).await;

            ////////////////////////////////////////////////////////////////////////
            // Step 2 - [NFTs] - Query NFT balance for address
            let nft_contract_address = config
                .get_network_by_chain_id(chain_id)
                .nft_contract_address;
            let parsed_body_response = non_fungible_token_balance(
                &config,
                &reqwest_client,
                &bearer_token,
                chain_id,
                config.funded_address.clone(),
                vec![nft_contract_address.clone()],
            )
            .await;

            print_with_time("[=] [NFT Balance] - Assert Response".to_string());

            assert!(!parsed_body_response["pagination"].is_null());

            let tokens = parsed_body_response["tokens"].as_array().unwrap();
            assert!(!tokens.is_empty());
            let nft = tokens.first().unwrap();

            assert_eq!("1", nft["balance"]);
            assert_eq!("TestNFT", nft["name"]);
            assert_eq!(nft_contract_address, nft["contract_address"]);

            assert!(nft["metadata"]["attributes"].as_array().unwrap().is_empty());
            assert_eq!(
                "A concise Hardhat tutorial Badge NFT with on-chain SVG images like look.",
                nft["metadata"]["description"]
            );
            assert!(!nft["metadata"]["image"].as_str().unwrap().is_empty());
            assert!(!nft["metadata"]["name"].as_str().unwrap().is_empty());

            print_with_time("[-] [NFT Balance] - Query NFT balance".to_string());
        },
        e2e_fixture,
    )
    .await;
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
async fn e2e_test_fetch_nft_balance_with_no_auth(
    e2e_fixture: &E2EFixture,
    _balance_fixture: &BalanceFixture,
) {
    run_e2e_test(
        "No Auth",
        |test_context| async move {
            let config = test_context.config;
            let chain_id = test_context.chain_id;
            let reqwest_client = test_context.client;

            let nft_contract_address = config
                .get_network_by_chain_id(chain_id)
                .nft_contract_address;

            print_with_time("[+] [NFT Balance] - Query NFT balance".to_string());

            let api_url = format!(
                "{}/api/v1/chains/{}/addresses/{}/tokens/nft/query",
                config.env_url,
                chain_id,
                config.funded_address.clone()
            );
            let response = post_without_retry(
                &reqwest_client,
                &api_url,
                "",
                json!({ "contract_addresses": [nft_contract_address] }).to_string(),
                "application/json",
            )
            .await;

            print_with_time("[=] [NFT Balance] - Assert Response".to_string());
            assert_eq!(StatusCode::UNAUTHORIZED, response.status());
        },
        e2e_fixture,
    )
    .await;
}
