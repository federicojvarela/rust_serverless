use crate::tools::fixtures::e2e::*;
use crate::tools::helper::*;
use crate::tools::runner::run_e2e_test;
use ethers::types::U256;
use rstest::rstest;

// ⚠️⚠️⚠️ println! should be replaced with report library in the future ⚠️⚠️⚠️

#[rstest]
#[tokio::test(flavor = "multi_thread")]
#[ignore]
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

    //////////////////////////////////////////////////////////////////////////
    //// Step 14 - [Cancellation] - Signs a LEGACY transaction with the newly created key
    //// Call the sign_legacy_transaction function to sign a LEGACY transaction in order to request
    //// a cancellation of that order.
    print_test_case_output_line_begins();
    let order_id_for_cancellation = sign_legacy_transaction(
        &config,
        &reqwest_client,
        &bearer_token,
        chain_id,
        address.clone(),
        U256::from(90), // arbitrary value to ensure it doesn't get process
    )
    .await;

    ////////////////////////////////////////////////////////////////////////
    //// Step 15 - [Cancellation] - wait until the transaction is received
    let _ = monitor_order_status_until_state(
        &config,
        &reqwest_client,
        &bearer_token,
        order_id_for_cancellation.clone(),
        RECEIVED_STATE,
    )
    .await;

    //////////////////////////////////////////////////////////////////////////
    //// Step 16 - [Cancellation] - Cancel the given order
    //// Call the cancel function to cancel the order_id.
    request_cancel_order_transaction_with_assertion(
        &config,
        &reqwest_client,
        &bearer_token,
        order_id_for_cancellation.clone(),
    )
    .await;

    //////////////////////////////////////////////////////////////////////////
    //// Step 17 - [Cancellation] - Validates Order Status
    //// Call the get_order_status function again to check the status of the CANCELLED order.
    let order_state_response = monitor_and_get_order_status(
        &config,
        &reqwest_client,
        &bearer_token,
        order_id_for_cancellation,
        "SIGNATURE_ORDER",
    )
    .await;
    assert_eq!(order_state_response["state"].as_str(), Some("CANCELLED"));

    //////////////////////////////////////////////////////////////////////////
    //// Step 18 - [Cancellation] - Signs an eip-1559 transaction with the newly created key
    //// Call the sign_1559_transaction function to sign an eip-1559 transaction in order to request
    //// a cancellation of that order.
    print_test_case_output_line_begins();
    let order_id_for_cancellation = sign_1559_transaction(
        &config,
        &reqwest_client,
        &bearer_token,
        chain_id,
        address.clone(),
        U256::from(1),
        U256::from(1),
    )
    .await;

    ////////////////////////////////////////////////////////////////////////
    //// Step 19 - [Cancellation] - wait until the transaction is received
    let _ = monitor_order_status_until_state(
        &config,
        &reqwest_client,
        &bearer_token,
        order_id_for_cancellation.clone(),
        RECEIVED_STATE,
    )
    .await;

    //////////////////////////////////////////////////////////////////////////
    //// Step 20 - [Cancellation] - Cancel the given order
    //// Call the cancel function to cancel the order_id.
    request_cancel_order_transaction_with_assertion(
        &config,
        &reqwest_client,
        &bearer_token,
        order_id_for_cancellation.clone(),
    )
    .await;

    //////////////////////////////////////////////////////////////////////////
    //// Step 21 - [Cancellation] - Validates Order Status
    //// Call the get_order_status function again to check the status of the CANCELLED order.
    let order_state_response = monitor_and_get_order_status(
        &config,
        &reqwest_client,
        &bearer_token,
        order_id_for_cancellation.clone(),
        "SIGNATURE_ORDER",
    )
    .await;
    assert_eq!(order_state_response["state"].as_str(), Some("CANCELLED"));

    //////////////////////////////////////////////////////////////////////
    // Step 22 - [Speed up order] - send a legacy transaction with a min gas price to avoid get mined
    print_test_case_output_line_begins();
    let legacy_transaction_order_id = sign_legacy_transaction(
        &config,
        &reqwest_client,
        &bearer_token,
        chain_id,
        address.clone(),
        U256::from(1),
    )
    .await;

    //////////////////////////////////////////////////////////////////////
    // Step 23 - [Speed up order] - wait until the transaction is submitted
    let parsed_response_body = monitor_order_status_until_state(
        &config,
        &reqwest_client,
        &bearer_token,
        legacy_transaction_order_id.clone(),
        SUBMITTED_STATE,
    )
    .await;

    let original_tx_hash = parsed_response_body["data"]["transaction_hash"].to_string();

    //////////////////////////////////////////////////////////////////////
    // Step 24 - [Speed up order] - send speedup request with new gas value
    request_speedup_legacy_transaction(
        &config,
        &reqwest_client,
        &bearer_token,
        legacy_transaction_order_id.clone(),
        gas_price,
        true,
    )
    .await;

    //////////////////////////////////////////////////////////////////////
    // Step 25 - [Speed up order] - check speed up order
    validate_speedup_order_status(
        &config,
        &reqwest_client,
        &bearer_token,
        legacy_transaction_order_id,
        original_tx_hash,
    )
    .await;

    ////////////////////////////////////////////////////////////////////////
    // Step 26 - [Speed up order] - send a eip-1559 transaction with a min gas price to avoid get mined
    print_test_case_output_line_begins();
    let transaction_1559_order_id = sign_1559_transaction(
        &config,
        &reqwest_client,
        &bearer_token,
        chain_id,
        address.clone(),
        U256::from(1),
        U256::from(1),
    )
    .await;

    //////////////////////////////////////////////////////////////////////
    // Step 27 - [Speed up order] - wait until the transaction is submitted
    let parsed_response_body = monitor_order_status_until_state(
        &config,
        &reqwest_client,
        &bearer_token,
        transaction_1559_order_id.clone(),
        SUBMITTED_STATE,
    )
    .await;
    let original_tx_hash = parsed_response_body["transaction_hash"].to_string();

    //////////////////////////////////////////////////////////////////////
    // Step 28 - [Speed up order] - send speedup request with new gas value
    request_speedup_1559_transaction(
        &config,
        &reqwest_client,
        &bearer_token,
        transaction_1559_order_id.clone(),
        max_fee,
        max_priority_fee,
    )
    .await;

    //////////////////////////////////////////////////////////////////////
    // Step 29 - [Speed up order] - check speed up order
    validate_speedup_order_status(
        &config,
        &reqwest_client,
        &bearer_token,
        transaction_1559_order_id,
        original_tx_hash,
    )
    .await;
}
