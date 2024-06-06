use crate::tools::fixtures::e2e::*;
use crate::tools::helper::*;
use crate::tools::runner::run_e2e_test;
use rstest::rstest;

#[rstest]
#[tokio::test(flavor = "multi_thread")]
async fn e2e_test_insufficient_funds(e2e_fixture: &E2EFixture) {
    run_e2e_test("Insufficient Funds", test_insufficient_funds, e2e_fixture).await;
}

async fn test_insufficient_funds(test_context: TestContext) {
    let config = test_context.config;
    let chain_id = test_context.chain_id;
    let reqwest_client = test_context.client;

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
    // Step missing on purpose - [nothing] - Do NOT fund the new address.
    // yes, a noop.

    ////////////////////////////////////////////////////////////////////////
    // Step 4 - [Get gas price prediction] - get gas prices
    let fees_response = get_gas_price_prediction(
        &config,
        &reqwest_client,
        &bearer_token,
        chain_id.to_string().as_str(),
        true,
    )
    .await;
    let (gas_price, max_fee, max_priority_fee) = parse_high_fees(fees_response.clone());

    ////////////////////////////////////////////////////////////////////////
    // Step 5 - [Sign] - Signs a LEGACY transaction with the newly created key
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
    // Step 6 - [Get order status] - Validates insufficient funds errors for a LEGACY transaction.
    let legacy_order = monitor_and_get_order_status(
        &config,
        &reqwest_client,
        &bearer_token,
        legacy_transaction_order_id,
        "SIGNATURE_ORDER",
    )
    .await;
    assert_eq!(legacy_order["state"], NOT_SUBMITTED_STATE);
    assert!(legacy_order["error"]
        .as_str()
        .unwrap_or("")
        .contains("insufficient funds"));

    ////////////////////////////////////////////////////////////////////////
    // Step 7 - [Sign] - Signs a LEGACY 1559 with the newly created key
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
    // Step 8 - [Get order status] - Validates insufficient funds errors for a LEGACY transaction.
    let transaction_1559_order = monitor_and_get_order_status(
        &config,
        &reqwest_client,
        &bearer_token,
        transaction_1559_order_id,
        "SIGNATURE_ORDER",
    )
    .await;
    assert_eq!(transaction_1559_order["state"], NOT_SUBMITTED_STATE);
    assert!(transaction_1559_order["error"]
        .as_str()
        .unwrap_or("")
        .contains("insufficient funds"));
}
