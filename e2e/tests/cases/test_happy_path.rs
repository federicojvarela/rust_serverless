use crate::tools::fixtures::e2e::*;
use crate::tools::helper::*;
use crate::tools::runner::run_e2e_test;
use rstest::rstest;

const ETHEREUM_TESTNET_SEPOLIA_CHAIN_ID: u64 = 11155111;

// ⚠️⚠️⚠️ println! should be replaced with report library in the future ⚠️⚠️⚠️

#[rstest]
#[tokio::test(flavor = "multi_thread")]
async fn e2e_test_happy_path(e2e_fixture: &E2EFixture) {
    run_e2e_test("Happy Path", test_happy_path, e2e_fixture).await;
}

async fn test_happy_path(test_context: TestContext) {
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
    let prediction_fees_response = get_gas_price_prediction(
        &config,
        &reqwest_client,
        &bearer_token,
        chain_id.to_string().as_str(),
        true,
    )
    .await;
    let (gas_price, max_fee, max_priority_fee) = parse_high_fees(prediction_fees_response.clone());

    ////////////////////////////////////////////////////////////////////////
    // Step 5 - [Funding] - Transfers funds to the newly created key from a previously funded one
    // Call the fund_new_address function to transfer funds to the newly created address.
    let (funding_order_id, funding_authorization) = fund_new_address(
        &config,
        &reqwest_client,
        &bearer_token,
        chain_id,
        gas_price,
        address.clone(),
    )
    .await;

    ////////////////////////////////////////////////////////////////////////
    // Step 6 - [Get order status] - Validates Order Status
    // Call the get_order_status function again to check the status of the funding order.
    let _ = monitor_funding_order_status_until_state(
        &config,
        &reqwest_client,
        &funding_authorization,
        funding_order_id.clone(),
        COMPLETED_STATE,
    )
    .await;

    ////////////////////////////////////////////////////////////////////////
    // Step 7 - [Sign] - Signs a LEGACY transaction with the newly created key
    // Call the sign_legacy_transaction function to sign a LEGACY transaction.
    print_test_case_output_line_begins();
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
    // Step 8 - [Get order status] - Validates Order Status
    // Call the get_order_status function again to check the status of the LEGACY transaction order.
    let _ = monitor_and_get_order_status(
        &config,
        &reqwest_client,
        &bearer_token,
        legacy_transaction_order_id,
        "SIGNATURE_ORDER",
    )
    .await;

    ////////////////////////////////////////////////////////////////////////
    // Step 9 - [Sign] - Signs a 1559 transaction with the newly created key
    // Call the sign_1559_transaction function to sign a 1559 transaction.
    print_test_case_output_line_begins();
    let transaction_1559_order_id = sign_1559_transaction(
        &config,
        &reqwest_client,
        &bearer_token,
        chain_id,
        address.clone(),
        max_fee,
        max_priority_fee,
    )
    .await;

    ////////////////////////////////////////////////////////////////////////
    // Step 10 - [Get order status] - Validates Order Status
    // Call the get_order_status function once more to check the status of the 1559 transaction order.
    let _ = monitor_and_get_order_status(
        &config,
        &reqwest_client,
        &bearer_token,
        transaction_1559_order_id,
        "SIGNATURE_ORDER",
    )
    .await;

    //////////////////////////////////////////////////////////////////////
    // Step 11 - [Get Native Token Balance] - Validate Current Balance of Native Tokens from created address.
    print_test_case_output_line_begins();
    native_token_balance(
        &config,
        &reqwest_client,
        &bearer_token,
        Some(chain_id),
        address.clone(),
    )
    .await;

    //////////////////////////////////////////////////////////////////////
    // Step 12 - [Get Fungible Token Balance] - Validate Current Balance of USDT contract from created address.
    fungible_token_balance(
        &config,
        &reqwest_client,
        &bearer_token,
        chain_id,
        address.clone(),
    )
    .await;

    //////////////////////////////////////////////////////////////////////
    // Step 13 - [Get Non Fungible Token Balance] - validate the current balance for the BAYC
    // contract for the created address.
    non_fungible_token_balance_is_empty(
        &config,
        &reqwest_client,
        &bearer_token,
        chain_id,
        address.clone(),
        vec![BORED_APES_CONTRACT.to_owned()],
    )
    .await;

    // Sponsored transactions are only available on Ethereum testnet Sepolia
    if chain_id == ETHEREUM_TESTNET_SEPOLIA_CHAIN_ID {
        print_test_case_output_line_begins();

        ////////////////////////////////////////////////////////////////////////
        // (Sponsored) Step 1 - [Create Key] - Creates a new gas pool to sign sponsored transactions with
        // let order_id = create_key(&config, &reqwest_client, &bearer_token).await;

        ////////////////////////////////////////////////////////////////////////
        // (Sponsored) Step 2 - [Get order status] - Validates Order Status
        // let gas_pool_address = monitor_and_get_address_order_status(
        //     &config,
        //     &reqwest_client,
        //     &bearer_token,
        //     order_id,
        //     "KEY_CREATION_ORDER",
        // )
        // .await;

        let gas_pool_address = config.gas_pool_address_e2e.clone();
        if gas_pool_address.is_empty() {
            panic!("Set env var GAS_POOL_ADDRESS_E2E with gas pool to be used in sponsored transactions.");
        }

        ////////////////////////////////////////////////////////////////////////
        // (Sponsored) Step 3 - [Funding] - Transfers funds to the gas pool from a previously funded one
        // Call the fund_gas_pool function to transfer funds to the gas pool address.
        let (funding_gas_pool_order_id, funding_gas_pool_authorization) = fund_gas_pool(
            &config,
            &reqwest_client,
            &bearer_token,
            chain_id,
            gas_price,
            gas_pool_address.clone(),
        )
        .await;

        ////////////////////////////////////////////////////////////////////////
        // Sponsored) Step 4 - [Get order status] - Validates Order Status
        // Call the get_order_status function again to check the status of the funding order.
        let _ = monitor_funding_order_status_until_state(
            &config,
            &reqwest_client,
            &funding_gas_pool_authorization,
            funding_gas_pool_order_id.clone(),
            COMPLETED_STATE,
        )
        .await;

        ////////////////////////////////////////////////////////////////////////
        // (Sponsored) Step 5 - [Create gas pool address] - Set gas pool address
        create_gas_pool(
            &config,
            &reqwest_client,
            &bearer_token,
            chain_id,
            gas_pool_address.clone(),
        )
        .await;

        ////////////////////////////////////////////////////////////////////////
        // (Sponsored) Step 6 - [Create gas pool address] - Set gas pool address
        // get_gas_pool(&config, &reqwest_client, &bearer_token, chain_id).await;

        ////////////////////////////////////////////////////////////////////////
        // Sponsored) Step 7 - [Mint tokens] - Mint tokens to the newly created account address
        // Call the sign function to call "mint() function in the token contract".
        let mint_call_order_id = mint_token(
            &config,
            &reqwest_client,
            &bearer_token,
            chain_id,
            address.clone(),
            max_fee,
            max_priority_fee,
        )
        .await;

        ////////////////////////////////////////////////////////////////////////
        // Sponsored) Step 8 - [Get order status] - Validates Order Status
        // Call the get_order_status function again to check the status of the token minting.
        let _ = monitor_funding_order_status_until_state(
            &config,
            &reqwest_client,
            &funding_gas_pool_authorization,
            mint_call_order_id.clone(),
            COMPLETED_STATE,
        )
        .await;

        ////////////////////////////////////////////////////////////////////////
        // Sponsored) Step 9 - [Sign Sponsored] - Signs a sponsored transaction with the newly created key
        // Call the sign_sponsored function to sign a sponsored transaction.
        let sponsored_transaction_order_id = sign_sponsored_transaction(
            &config,
            &reqwest_client,
            &bearer_token,
            chain_id,
            address.clone(),
        )
        .await;

        ////////////////////////////////////////////////////////////////////////
        //// Sponsored) Step 10 - [Sponsored order] - wait until the transaction is received
        let _ = monitor_order_status_until_state(
            &config,
            &reqwest_client,
            &bearer_token,
            sponsored_transaction_order_id,
            COMPLETED_STATE,
        )
        .await;
    }
}
