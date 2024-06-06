use crate::tools::fixtures::e2e::*;
use crate::tools::helper::*;
use crate::tools::runner::run_e2e_test;
use rstest::rstest;

// ⚠️⚠️⚠️ println! should be replaced with report library in the future ⚠️⚠️⚠️

#[rstest]
#[tokio::test(flavor = "multi_thread")]
async fn e2e_test_multiple_orders_happy_path(e2e_fixture: &E2EFixture) {
    run_e2e_test(
        "Multiple Orders Happy Path",
        test_multiple_orders_happy_path,
        e2e_fixture,
    )
    .await;
}

async fn test_multiple_orders_happy_path(test_context: TestContext) {
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
    let (gas_price, max_fee, max_priority_fee) = parse_high_fees(historical_fees_response.clone());

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
    // Step 7 - [Sign] - Signs two LEGACY transaction with the newly created key
    // Call the sign_legacy_transaction function to sign a LEGACY transaction.
    let legacy_transaction_order_id_1 = sign_legacy_transaction(
        &config,
        &reqwest_client,
        &bearer_token,
        chain_id,
        address.clone(),
        gas_price,
    )
    .await;

    // TODO: Remove this once the lock is implemented in the order selector
    tokio::time::sleep(std::time::Duration::from_secs(10)).await;

    let legacy_transaction_order_id_2 = sign_legacy_transaction(
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
    let (order_1, order_2) = tokio::join!(
        monitor_and_get_order_status(
            &config,
            &reqwest_client,
            &bearer_token,
            legacy_transaction_order_id_1,
            "SIGNATURE_ORDER",
        ),
        monitor_and_get_order_status(
            &config,
            &reqwest_client,
            &bearer_token,
            legacy_transaction_order_id_2,
            "SIGNATURE_ORDER",
        )
    );

    assert_eq!(order_1["state"], COMPLETED_STATE);
    assert_eq!(order_2["state"], COMPLETED_STATE);

    ////////////////////////////////////////////////////////////////////////
    // Step 9 - [Sign] - Signs a 1559 transaction with the newly created key
    // Call the sign_1559_transaction function to sign a 1559 transaction.
    let transaction_1559_order_id_1 = sign_1559_transaction(
        &config,
        &reqwest_client,
        &bearer_token,
        chain_id,
        address.clone(),
        max_fee,
        max_priority_fee,
    )
    .await;

    // TODO: Remove this once the lock is implemented in the order selector
    tokio::time::sleep(std::time::Duration::from_secs(10)).await;

    let transaction_1559_order_id_2 = sign_1559_transaction(
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
    let (order_1, order_2) = tokio::join!(
        monitor_and_get_order_status(
            &config,
            &reqwest_client,
            &bearer_token,
            transaction_1559_order_id_1,
            "SIGNATURE_ORDER",
        ),
        monitor_and_get_order_status(
            &config,
            &reqwest_client,
            &bearer_token,
            transaction_1559_order_id_2,
            "SIGNATURE_ORDER",
        ),
    );

    assert_eq!(order_1["state"], COMPLETED_STATE);
    assert_eq!(order_2["state"], COMPLETED_STATE);
}
