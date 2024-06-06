// TODO: When uningnoring this test put the approvers names in eph variables.
// Policy names cannot be the same across domains due to a Maestro bug
use crate::tools::fixtures::e2e::*;
use crate::tools::helper::*;
use crate::tools::runner::run_e2e_test;
use common::test_tools::http::constants::ADDRESS_FOR_MOCK_REQUESTS;
use ethers::types::U256;
use rstest::rstest;

// ⚠️⚠️⚠️ println! should be replaced with report library in the future ⚠️⚠️⚠️

const APPROVED_STATE: &str = "APPROVED";
const APPROVERS_REVIEWED_STATE: &str = "APPROVERS_REVIEWED";
const ADDRESS_WITH_CUSTOM_POLICY: &str = ADDRESS_FOR_MOCK_REQUESTS;

#[rstest]
#[tokio::test(flavor = "multi_thread")]
#[ignore = "unignore when bootstrap is done"]
async fn different_policies(e2e_fixture: &E2EFixture) {
    run_e2e_test("Default Policy", default_policy, e2e_fixture).await;
    run_e2e_test("Custom Policy", custom_policy, e2e_fixture).await;
}

async fn default_policy(test_context: TestContext) {
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
    // Step 4 - [Sign] - Signs a 1559 transaction with the newly created key
    // Call the sign_1559_transaction function to sign a 1559 transaction.
    let order_id = sign_1559_transaction(
        &config,
        &reqwest_client,
        &bearer_token,
        chain_id,
        address.clone(),
        U256::from(0),
        U256::from(0),
    )
    .await;

    ////////////////////////////////////////////////////////////////////////
    //// Step 5 - wait until the transaction is reviewed by approvers
    let order = monitor_order_status_until_state(
        &config,
        &reqwest_client,
        &bearer_token,
        order_id.clone(),
        APPROVERS_REVIEWED_STATE,
    )
    .await;

    ////////////////////////////////////////////////////////////////////////
    //// Step - Check the approver authority was the default one
    assert_eq!(
        order["data"]["approvals"][&config.default_approver_name],
        APPROVED_STATE,
    );
}

async fn custom_policy(test_context: TestContext) {
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
    // Step 4 - [Sign] - Signs a 1559 transaction with the newly created key
    // Call the sign_1559_transaction function to sign a 1559 transaction.
    let order_id = sign_1559_transaction_to_specific_address(
        &config,
        &reqwest_client,
        &bearer_token,
        chain_id,
        address.clone(),
        ADDRESS_WITH_CUSTOM_POLICY.to_owned(),
        U256::from(0),
        U256::from(0),
    )
    .await;

    ////////////////////////////////////////////////////////////////////////
    //// Step 5 - wait until the transaction is reviewed by approvers
    let order = monitor_order_status_until_state(
        &config,
        &reqwest_client,
        &bearer_token,
        order_id.clone(),
        APPROVERS_REVIEWED_STATE,
    )
    .await;

    ////////////////////////////////////////////////////////////////////////
    //// Step - Check the approver authority was the default one
    assert_eq!(
        order["data"]["approvals"][&config.default_approver_name],
        APPROVED_STATE,
    );
    assert_eq!(
        order["data"]["approvals"][&config.custom_approver_name],
        APPROVED_STATE,
    );
}
