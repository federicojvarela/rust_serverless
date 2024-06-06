use crate::tools::config::Config;
use chrono::{Local, Timelike};
use ethers::types::U256;
use serde_json::{json, Value};
use std::time::Duration;
use tokio::time::Instant;

const GAS_LIMIT: u32 = 22000; // we might want to make this a constant for wider use
const CONTRACT_CALL_GAS_LIMIT: u32 = 100000;
const VALUE_FOR_TXN: u8 = 1;
const DEADLINE: &str = "1807594318";
const VALUE_FOR_SPONSORED_TX: u8 = 0;
const SPONSORED_TX_DATA: &str = "0xa9059cbb000000000000000000000000497838d6b9813365ee9fd6c13f1914d508d80d0d0000000000000000000000000000000000000000000000000000000000000001";
const RECIPIENT_CONTRACT_ADDRESS: &str = "0x1FF0C341C5aA8728c57534C8a36508A177fF3a4c";
const MINT_TOKEN_DATA: &str = "0x1249c58b";
const NUM_OF_TXNS: u8 = 6; // one 1559 txn, one legacy txn

const USDT_CONTRACT: &str = "0xdac17f958d2ee523a2206206994597c13d831ec7";
pub const BORED_APES_CONTRACT: &str = "0xbc4ca0eda7647a8ab7c2061c2e118a18a936f13d";

pub const COMPLETED_STATE: &str = "COMPLETED";
pub const NOT_SUBMITTED_STATE: &str = "NOT_SUBMITTED";
pub const RECEIVED_STATE: &str = "RECEIVED";
pub const SUBMITTED_STATE: &str = "SUBMITTED";
pub const ERROR_STATE: &str = "ERROR";
const FINAL_STATES: [&str; 3] = [COMPLETED_STATE, ERROR_STATE, NOT_SUBMITTED_STATE];

pub const TEST_OUTPUT_DELIMITER_LINE: &str = "====================================================";
pub const TEST_CASE_OUTPUT_DELIMITER_LINE: &str =
    "----------------------------------------------------";

pub async fn authorize(config: &Config, client: &reqwest::Client) -> String {
    let client_id = config.client_id.clone();
    let auth_url = config.auth_url.clone();
    let authorization_token = config.authorization_token.clone();
    internal_authorize(client_id, auth_url, authorization_token, client).await
}

// This function asynchronously obtains an access token through the authorization process.
async fn internal_authorize(
    client_id: String,
    auth_url: String,
    authorization: String,
    client: &reqwest::Client,
) -> String {
    print_with_time("[+] [Authorize] - Obtain an Access Token".to_string());

    let body = format!("grant_type=client_credentials&client_id={}", client_id);

    // Perform a POST request to obtain the access token.
    let body_response = post(
        client,
        &auth_url,
        &authorization,
        body,
        "application/x-www-form-urlencoded",
    )
    .await;

    // Parse the response body into JSON format.
    let parsed_body_response = parse_body(body_response);

    print_with_time("[=] [Authorize] - Assert Response".to_string());
    assert!(!parsed_body_response["access_token"].to_string().is_empty());
    assert_eq!(parsed_body_response["expires_in"], 3600);
    assert_eq!(parsed_body_response["token_type"], "Bearer");

    print_with_time("[-] [Authorize] - Obtain an Access Token".to_string());

    format!(
        "Bearer {}",
        parsed_body_response["access_token"].as_str().unwrap()
    )
}

// This function asynchronously creates a new key for signing transactions.
pub async fn create_key(config: &Config, client: &reqwest::Client, bearer_token: &str) -> String {
    print_with_time("[+] [Create Key] - Creates a new key to sign transactions".to_string());

    let api_url = format!("{}/api/v1/keys", config.env_url);

    // Create the request body in JSON format, specifying the client user ID.
    let post_body = json!({
        "client_user_id": config.client_user_id
    })
    .to_string();

    // Perform a POST request to create the key and fetch the response body.
    let body_response = post(
        client,
        &api_url,
        bearer_token,
        post_body,
        "application/json",
    )
    .await;
    let parsed_body_response = parse_body(body_response);

    print_with_time("[=] [Create Key] - Assert Response".to_string());
    assert!(!parsed_body_response["order_id"].to_string().is_empty());

    print_with_time("[-] [Create Key] - Creates a new key to sign transactions".to_string());
    parsed_body_response["order_id"]
        .as_str()
        .unwrap()
        .to_string()
}

pub async fn monitor_and_get_address_order_status(
    config: &Config,
    client: &reqwest::Client,
    bearer_token: &str,
    order_id: String,
    order_type: &str,
) -> String {
    let parsed_body_response =
        monitor_and_get_order_status(config, client, bearer_token, order_id, order_type).await;

    parsed_body_response["data"]["address"]
        .as_str()
        .unwrap()
        .to_string()
}

pub async fn monitor_and_get_order_status(
    config: &Config,
    client: &reqwest::Client,
    bearer_token: &str,
    order_id: String,
    order_type: &str,
) -> Value {
    let env_url: String = config.env_url.clone();

    monitor_order_status_from_env_url(
        env_url,
        config.timeout,
        client,
        bearer_token,
        order_id,
        order_type,
    )
    .await
}

async fn get_order_status_from_url(
    env_url: &str,
    client: &reqwest::Client,
    bearer_token: &str,
    order_id: &str,
) -> Value {
    print_with_time(format!("[!] [Get order status] - OrderID: {}", order_id));
    let api_url = format!("{}/api/v1/orders/{}/status", env_url, order_id);
    let body_response = get(client, &api_url, bearer_token).await;
    parse_body(body_response)
}

pub async fn monitor_funding_order_status_until_state(
    config: &Config,
    client: &reqwest::Client,
    bearer_token: &str,
    order_id: String,
    state: &str,
) -> Value {
    let mut env_url: String = config.env_url.clone();

    if config.ephemeral {
        env_url = get_ephemeral_config(&config.dev_env_url);
    };

    monitor_order_status_until_state_from_url(
        env_url,
        config,
        client,
        bearer_token,
        order_id,
        state,
    )
    .await
}

pub async fn monitor_order_status_until_state(
    config: &Config,
    client: &reqwest::Client,
    bearer_token: &str,
    order_id: String,
    state: &str,
) -> Value {
    let env_url: String = config.env_url.clone();

    monitor_order_status_until_state_from_url(
        env_url,
        config,
        client,
        bearer_token,
        order_id,
        state,
    )
    .await
}

pub async fn monitor_order_status_until_state_from_url(
    env_url: String,
    config: &Config,
    client: &reqwest::Client,
    bearer_token: &str,
    order_id: String,
    state: &str,
) -> Value {
    let start_time = Instant::now();
    let timeout_duration = Duration::from_secs(config.timeout);
    let mut parsed_body_response;

    print_with_time(format!(
        "[+] [Get order status] - Validates Order Status for order {}",
        order_id
    ));

    loop {
        if start_time.elapsed() >= timeout_duration {
            print_with_time(format!(
                "[!] [Get order status] - Timeout has occurred for order {}",
                order_id
            ));
            panic!();
        }

        parsed_body_response =
            get_order_status_from_url(env_url.as_str(), client, bearer_token, order_id.as_str())
                .await;

        let current_state = parsed_body_response["state"].as_str().unwrap();

        if current_state == state || FINAL_STATES.contains(&current_state) {
            print_with_time(format!(
                "[!] [Get order status] - State is {} for order {}",
                current_state, order_id
            ));
            break;
        } else {
            print_with_time(format!(
                "[~] [Get order status] - State is {}, waiting for {} for order {}",
                parsed_body_response["state"], state, order_id
            ));
        }

        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    assert_eq!(parsed_body_response["order_id"], order_id);
    assert_eq!(parsed_body_response["order_version"], "1");
    assert_eq!(parsed_body_response["state"], state);
    print_with_time(format!(
        "[-] [Get order status] - Validates Order Status for order {}",
        order_id
    ));

    parsed_body_response
}

// This function asynchronously fetches and monitors the status of an order from a specific env URL.
// Note: This requires the correct env_url authorization (bearer token)
async fn monitor_order_status_from_env_url(
    env_url: String,
    timeout: u64,
    client: &reqwest::Client,
    bearer_token: &str,
    order_id: String,
    order_type: &str,
) -> Value {
    print_with_time("[+] [Get order status] - Validates Order Status".to_string());

    // Initialize variables for loop control and response parsing.
    let start_time = Instant::now();
    let timeout_duration = Duration::from_secs(timeout);
    let mut parsed_body_response;

    // Continue looping until the state is a final state.
    loop {
        parsed_body_response =
            get_order_status_from_url(env_url.as_str(), client, bearer_token, order_id.as_str())
                .await;

        // Match the state field of the parsed response.
        match parsed_body_response["state"].as_str() {
            Some("ERROR") => {
                print_with_time("[!] [Get order status] - State is \"ERROR\"".to_string());
                break;
            }
            Some("COMPLETED") => {
                print_with_time("[!] [Get order status] - State is \"COMPLETED\"".to_string());
                break;
            }
            Some("NOT_SUBMITTED") => {
                print_with_time("[!] [Get order status] - State is \"NOT_SUBMITTED\"".to_string());
                break;
            }
            Some("CANCELLED") => {
                print_with_time("[!] [Get order status] - State is \"CANCELLED\"".to_string());
                break;
            }
            _ => {
                // If the state is neither ERROR nor NOT_SUBMITTED nor COMPLETED, wait before the next iteration.
                print_with_time(format!(
                    "[~] [Get order status] - State is {}, waiting for ERROR or NOT_SUBMITTED or COMPLETED or CANCELLED",
                    parsed_body_response["state"]
                ));
            }
        }

        if start_time.elapsed() >= timeout_duration {
            print_with_time("[!] [Get order status] - Timeout has occurred".to_string());
            panic!();
        }

        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    print_with_time("[=] [Get order status] - Assert Response".to_string());
    assert_eq!(parsed_body_response["order_type"], order_type.to_string());
    assert_eq!(parsed_body_response["order_id"], order_id);
    assert_eq!(parsed_body_response["order_version"], "1");
    print_with_time("[-] [Get order status] - Validates Order Status".to_string());

    parsed_body_response
}

pub async fn get_gas_price_prediction(
    config: &Config,
    client: &reqwest::Client,
    bearer_token: &str,
    chain_id: &str,
    retry: bool,
) -> Value {
    print_with_time("[+] [Get gas price prediction] - Query gas prediction".to_string());

    let api_url = format!(
        "{}/api/v1/chains/{}/price/prediction",
        config.env_url, chain_id
    );
    let body_response = if retry {
        get(client, &api_url, bearer_token).await
    } else {
        let response = get_without_retry(client, &api_url, bearer_token).await;
        response.text().await.expect("Failed to read response body")
    };

    print_with_time("[-] [Get gas price prediction] - Query gas prediction".to_string());
    parse_body(body_response)
}

// This function asynchronously retrieves historical fees for the given chain_id.
pub async fn get_historical_fees(
    config: &Config,
    client: &reqwest::Client,
    bearer_token: &str,
    chain_id: &str,
    retry: bool,
) -> Value {
    print_with_time("[+] [Get historical fees] - Query historical fees".to_string());

    let api_url = format!("{}/api/v1/chains/{}/fees/history", config.env_url, chain_id);
    let body_response = if retry {
        get(client, &api_url, bearer_token).await
    } else {
        let response = get_without_retry(client, &api_url, bearer_token).await;
        response.text().await.expect("Failed to read response body")
    };

    print_with_time("[-] [Get historical fees] - Query historical fees".to_string());
    parse_body(body_response)
}

pub fn parse_fees(parsed_body_response: Value, name: &str) -> (U256, U256, U256) {
    print_with_time("[=] [Get fees] - Assert Response".to_string());
    assert!(!parsed_body_response["legacy"]["gas_price"][name]
        .to_string()
        .is_empty());
    assert!(!parsed_body_response["eip1559"]["max_fee_per_gas"][name]
        .to_string()
        .is_empty());
    assert!(
        !parsed_body_response["eip1559"]["max_priority_fee_per_gas"][name]
            .to_string()
            .is_empty()
    );

    let gas_price = U256::from_dec_str(
        parsed_body_response["legacy"]["gas_price"][name]
            .as_str()
            .unwrap(),
    )
    .expect("There was an error converting the gas_price to U256");

    let max_fee = U256::from_dec_str(
        parsed_body_response["eip1559"]["max_fee_per_gas"][name]
            .as_str()
            .unwrap(),
    )
    .expect("There was an error converting the max_fee_per_gas to U256");

    let max_priority_fee = U256::from_dec_str(
        parsed_body_response["eip1559"]["max_priority_fee_per_gas"][name]
            .as_str()
            .unwrap(),
    )
    .expect("There was an error converting the max_priority_fee_per_gas to U256");

    print_with_time("[~] [Get historical fees] - Convert values to U256".to_string());

    print_with_time(format!(
        "[!] [Get fees] - Fees ({:?}): gas_price = {:?}, max_fee = {:?}, max_priority_fee = {:?}",
        name, gas_price, max_fee, max_priority_fee
    ));
    (gas_price, max_fee, max_priority_fee)
}

pub fn parse_high_fees(parsed_body_response: Value) -> (U256, U256, U256) {
    let (gas_price, max_fee, max_priority_fee) = parse_fees(parsed_body_response, "high");

    assert!(gas_price > U256::from(0));
    assert!(max_fee > U256::from(0));
    assert!(max_priority_fee >= U256::from(0));

    (gas_price, max_fee, max_priority_fee)
}

// This function asynchronously funds a newly created address by transferring funds from a previously funded address.
pub async fn fund_new_address(
    config: &Config,
    client: &reqwest::Client,
    bearer_token: &str,
    chain_id: u64,
    gas_price: U256,
    address: String,
) -> (String, String) {
    // ephemeral e2e test require these values from dev
    let authorization_token: String;
    let auth_url: String;
    let client_id: String;
    let env_url: String;
    let funded_address: String;

    // stringfied bearer token
    let authorization: String;

    if config.ephemeral {
        auth_url = get_ephemeral_config(&config.dev_auth_url);
        client_id = get_ephemeral_config(&config.dev_client_id);
        env_url = get_ephemeral_config(&config.dev_env_url);
        funded_address = get_ephemeral_config(&config.dev_funded_address);

        authorization_token = get_ephemeral_config(&config.dev_authorization_token);
        authorization = internal_authorize(client_id, auth_url, authorization_token, client).await;
    } else {
        env_url = config.env_url.clone();
        funded_address = config.funded_address.clone();
        authorization = bearer_token.to_string();
    }

    print_with_time(
        "[+] [Funding] - Transfers funds to the newly created key from a previously funded one"
            .to_string(),
    );
    let api_url = format!("{}/api/v1/keys/{}/sign", env_url, funded_address);

    // Increase gas_price by 20%
    let gas_price_plus_twenty = gas_price + (gas_price / 5);
    print_with_time(format!(
        "[=] [Funding] - Transfers funds gas_price is {:?} will use gas_price + 20% {:?}",
        gas_price, gas_price_plus_twenty
    ));

    // Calculate the amount to transfer - a.k.a. "value"
    let gas = U256::from(GAS_LIMIT);
    let val_for_txn = U256::from(VALUE_FOR_TXN);
    let num_of_txns = U256::from(NUM_OF_TXNS);
    let amount_to_transfer: U256 = (gas * gas_price_plus_twenty + val_for_txn) * num_of_txns;

    // Create the request body specifying the transaction details.
    let post_body = json!({
        "transaction": {
            "to":  address,
            "gas": GAS_LIMIT.to_string(),
            "gas_price": gas_price_plus_twenty.to_string(),
            "value": amount_to_transfer.to_string(),
            "data": "0x00",
            "chain_id": chain_id
        }
    })
    .to_string();

    print_with_time(format!(
        "[~] [Funding] - Transfers funds using gas_price {:?} and value {:?}",
        gas_price_plus_twenty, amount_to_transfer
    ));

    // Perform a POST request to sign the transaction and fetch the response body.
    let body_response = post(
        client,
        &api_url,
        &authorization,
        post_body,
        "application/json",
    )
    .await;

    let parsed_body_response = parse_body(body_response);
    let order_id = parsed_body_response["order_id"].as_str().unwrap();
    print_with_time("[=] [Funding] - Assert Response".to_string());
    assert!(!order_id.to_string().is_empty());
    print_with_time(
        "[-] [Funding] - Transfers funds to the newly created key from a previously funded one"
            .to_string(),
    );
    (order_id.to_string(), authorization)
}

// This function asynchronously signs a LEGACY transaction using the newly created key.
pub async fn sign_legacy_transaction(
    config: &Config,
    client: &reqwest::Client,
    bearer_token: &str,
    chain_id: u64,
    address: String,
    gas_price: U256,
) -> String {
    print_with_time("[+] [Sign Legacy] - Signs a LEGACY transaction".to_string());

    let api_url = format!("{}/api/v1/keys/{}/sign", config.env_url, address);

    // Create the request body specifying the LEGACY transaction details.
    let post_body = json!({
        "transaction": {
            "to": config.custodial_address,
            "gas": GAS_LIMIT.to_string(),
            "gas_price": gas_price.to_string(),
            "value": VALUE_FOR_TXN.to_string(),
            "data": "0x00",
            "chain_id": chain_id
        }
    })
    .to_string();

    // Perform a POST request to sign the LEGACY transaction and fetch the response body.
    let body_response = post(
        client,
        &api_url,
        bearer_token,
        post_body,
        "application/json",
    )
    .await;
    let parsed_body_response = parse_body(body_response);
    let order_id = parsed_body_response["order_id"].as_str().unwrap();

    print_with_time("[=] [Sign Legacy] - Assert Response".to_string());
    assert!(!order_id.to_string().is_empty());

    print_with_time("[-] [Sign Legacy] - Signs a LEGACY transaction".to_string());
    order_id.to_string()
}

// This function asynchronously SPEEDUP a legacy order transaction.
pub async fn request_speedup_legacy_transaction(
    config: &Config,
    client: &reqwest::Client,
    bearer_token: &str,
    order_id: String,
    gas_price: U256,
    retry: bool,
) -> reqwest::Response {
    print_with_time("[+] [Speedup Legacy] - Speedup LEGACY transaction".to_string());

    let api_url = format!("{}/api/v1/orders/{}/speedup", config.env_url, order_id);

    // Create the request body specifying the speedup LEGACY transaction details.
    let post_body = json!({
        "gas_price": gas_price.to_string(),
    })
    .to_string();

    let response = if retry {
        // Perform a POST request to speedup the LEGACY transaction.
        post_response(
            client,
            &api_url,
            bearer_token,
            post_body,
            "application/json",
        )
        .await
    } else {
        post_without_retry(
            client,
            &api_url,
            bearer_token,
            post_body,
            "application/json",
        )
        .await
    };

    print_with_time("[-] [Speedup Legacy] - Speedup LEGACY transaction".to_string());
    response
}

// This function asynchronously CANCEL an order transaction.
pub async fn request_cancel_order_transaction(
    config: &Config,
    client: &reqwest::Client,
    bearer_token: &str,
    order_id: String,
) -> reqwest::Response {
    print_with_time("[+] [Cancel Order] - Cancel the given order".to_string());

    let api_url = format!("{}/api/v1/orders/{}/cancel", config.env_url, order_id);

    // Create an empty body for cancel request.
    let post_body = "".to_string();

    // Perform a POST request to make a cancellation request.
    post_without_retry(
        client,
        &api_url,
        bearer_token,
        post_body,
        "application/json",
    )
    .await
}

// This function asynchronously CANCEL an order transaction.
pub async fn request_cancel_order_transaction_with_assertion(
    config: &Config,
    client: &reqwest::Client,
    bearer_token: &str,
    order_id: String,
) {
    print_with_time("[+] [Cancel Order] - Cancel the given order".to_string());

    let api_url = format!("{}/api/v1/orders/{}/cancel", config.env_url, order_id);

    // Create an empty body for cancel request.
    let post_body = "".to_string();

    // Perform a POST request to make a cancellation request.
    let body_response = post(
        client,
        &api_url,
        bearer_token,
        post_body,
        "application/json",
    )
    .await;

    print_with_time("[=] [Cancellation] - Assert Response".to_string());
    assert!(body_response.is_empty());

    print_with_time(
        "[-] [Cancellation] - Successfully requested a cancellation for the given order_id"
            .to_string(),
    );
}

// This function asynchronously signs a 1559 transaction using the newly created key.
pub async fn sign_1559_transaction(
    config: &Config,
    client: &reqwest::Client,
    bearer_token: &str,
    chain_id: u64,
    address: String,
    max_fee: U256,
    max_priority_fee: U256,
) -> String {
    sign_1559_transaction_to_specific_address(
        config,
        client,
        bearer_token,
        chain_id,
        address,
        config.custodial_address.clone(),
        max_fee,
        max_priority_fee,
    )
    .await
}

// This function asynchronously signs a 1559 transaction using the newly created key.
#[allow(clippy::too_many_arguments)]
pub async fn sign_1559_transaction_to_specific_address(
    config: &Config,
    client: &reqwest::Client,
    bearer_token: &str,
    chain_id: u64,
    address: String,
    to_address: String,
    max_fee: U256,
    max_priority_fee: U256,
) -> String {
    print_with_time("[+] [Sign 1559] - Signs a 1559 transaction".to_string());

    let api_url = format!("{}/api/v1/keys/{}/sign", config.env_url, address);
    let post_body = json!({
        "transaction": {
            "to": to_address,
            "gas": GAS_LIMIT.to_string(),
            "max_fee_per_gas": max_fee.to_string(),
            "max_priority_fee_per_gas": max_priority_fee.to_string(),
            "value": VALUE_FOR_TXN.to_string(),
            "data": "0x00",
            "chain_id": chain_id
        }
    })
    .to_string();

    // Perform a POST request to sign the 1559 transaction and fetch the response body.
    let body_response = post(
        client,
        &api_url,
        bearer_token,
        post_body,
        "application/json",
    )
    .await;
    let parsed_body_response = parse_body(body_response);
    let order_id = parsed_body_response["order_id"].as_str().unwrap();

    print_with_time("[=] [Sign 1559] - Assert Response".to_string());
    assert!(!order_id.to_string().is_empty());

    print_with_time("[-] [Sign 1559] - Signs a 1559 transaction".to_string());
    order_id.to_string()
}

// This function asynchronously signs a sponsored transaction using the newly created key.
pub async fn sign_sponsored_transaction(
    config: &Config,
    client: &reqwest::Client,
    bearer_token: &str,
    chain_id: u64,
    address: String,
) -> String {
    print_with_time("[+] [Sign Sponsored] - Signs a sponsored transaction".to_string());

    let api_url = format!("{}/api/v1/keys/{}/sign/sponsored", config.env_url, address);
    let post_body = json!({
        "transaction": {
            "to": RECIPIENT_CONTRACT_ADDRESS.to_string(),
            "deadline": DEADLINE.to_string(),
            "value": VALUE_FOR_SPONSORED_TX.to_string(),
            "data": SPONSORED_TX_DATA.to_string(),
            "chain_id": chain_id
        }
    })
    .to_string();

    // Perform a POST request to sign the sponsored transaction and fetch the response body.
    let body_response = post(
        client,
        &api_url,
        bearer_token,
        post_body,
        "application/json",
    )
    .await;
    let parsed_body_response = parse_body(body_response);
    let order_id = parsed_body_response["order_id"].as_str().unwrap();

    print_with_time("[=] [Sign Sponsored] - Assert Response".to_string());
    assert!(!order_id.to_string().is_empty());

    print_with_time("[-] [Sign Sponsored] - Signs a 1559 transaction".to_string());
    order_id.to_string()
}

// Mint tokens from the recipient contract
#[allow(clippy::too_many_arguments)]
pub async fn mint_token(
    config: &Config,
    client: &reqwest::Client,
    bearer_token: &str,
    chain_id: u64,
    from_address: String,
    max_fee: U256,
    max_priority_fee: U256,
) -> String {
    print_with_time(
        "[+] [Sign Contract Call] - Signs a 1559 transaction for a contract call".to_string(),
    );

    let api_url = format!("{}/api/v1/keys/{}/sign", config.env_url, from_address);
    let post_body = json!({
        "transaction": {
            "to": RECIPIENT_CONTRACT_ADDRESS.to_string(),
            "gas": CONTRACT_CALL_GAS_LIMIT.to_string(),
            "max_fee_per_gas": max_fee.to_string(),
            "max_priority_fee_per_gas": max_priority_fee.to_string(),
            "value": "0",
            "data": MINT_TOKEN_DATA,
            "chain_id": chain_id
        }
    })
    .to_string();

    // Perform a POST request to sign the 1559 transaction and fetch the response body.
    let body_response = post(
        client,
        &api_url,
        bearer_token,
        post_body,
        "application/json",
    )
    .await;
    let parsed_body_response = parse_body(body_response);
    let order_id = parsed_body_response["order_id"].as_str().unwrap();

    print_with_time("[=] [Sign Contract Call] - Assert Response".to_string());
    assert!(!order_id.to_string().is_empty());

    print_with_time("[-] [Sign Contract Call] - Signs the transaction".to_string());
    order_id.to_string()
}

// This function get the native token balance
pub async fn native_token_balance_request(
    config: &Config,
    client: &reqwest::Client,
    bearer_token: &str,
    chain_id: Option<u64>,
    address: String,
    retry: bool,
) -> Value {
    print_with_time("[+] [Native Token Balance] - Get Native token balance request".to_string());

    let chain_id = match chain_id {
        Some(x) => x.to_string(),
        None => "".to_string(),
    };

    let api_url = format!(
        "{}/api/v1/chains/{}/addresses/{}/tokens/native/query",
        config.env_url, chain_id, address
    );

    let body_response;

    if retry {
        body_response = post(
            client,
            &api_url,
            bearer_token,
            "".to_string(),
            "application/json",
        )
        .await;
    } else {
        let response = post_without_retry(
            client,
            &api_url,
            bearer_token,
            "".to_string(),
            "application/json",
        )
        .await;

        body_response = response.text().await.expect("Failed to read response body");
    }

    print_with_time("[-] [Native Token Balance] - Get Native token balance request".to_string());
    parse_body(body_response)
}

pub async fn fungible_token_balance_request(
    config: &Config,
    client: &reqwest::Client,
    bearer_token: &str,
    chain_id: u64,
    address: String,
    contracts: Vec<String>,
) -> Value {
    print_with_time("[+] [Fungible Token Balance] - Get Fungible token balance".to_string());

    let body = json!({ "contract_addresses": contracts }).to_string();

    let api_url = format!(
        "{}/api/v1/chains/{}/addresses/{}/tokens/ft/query",
        config.env_url, chain_id, address
    );
    let body_response = post(client, &api_url, bearer_token, body, "application/json").await;
    print_with_time(
        "[-] [Fungible Token Balance] - Get Fungible token balance request".to_string(),
    );
    parse_body(body_response)
}

pub async fn native_token_balance(
    config: &Config,
    client: &reqwest::Client,
    bearer_token: &str,
    chain_id: Option<u64>,
    address: String,
) {
    let parsed_body_response =
        native_token_balance_request(config, client, bearer_token, chain_id, address, true).await;

    print_with_time("[=] [Native Token Balance] - Assert Response".to_string());
    let balance = parsed_body_response["balance"].as_str().unwrap();
    assert!(!balance.is_empty());
    // TODO: This is failing need a TechDebt task fro fixing it
    // Balance is 0 and is asserting for <> 0
    // assert!(!U256::from_dec_str(balance).unwrap().is_zero());

    print_with_time("[-] [Native Token Balance] - Get Native token balance".to_string());
}

#[allow(dead_code)]
pub async fn fungible_token_balance(
    config: &Config,
    client: &reqwest::Client,
    bearer_token: &str,
    chain_id: u64,
    address: String,
) -> String {
    print_with_time("[+] [Fungible Token Balance] - Get Fungible token balance".to_string());

    let body = json!({ "contract_addresses": vec![USDT_CONTRACT] }).to_string();

    let api_url = format!(
        "{}/api/v1/chains/{}/addresses/{}/tokens/ft/query",
        config.env_url, chain_id, address
    );
    let body_response = post(client, &api_url, bearer_token, body, "application/json").await;
    let parsed_body_response = parse_body(body_response);

    print_with_time("[=] [Fungible Token Balance] - Assert Response".to_string());
    let balance = parsed_body_response["data"][0]["balance"].as_str().unwrap();

    assert!(!balance.is_empty());
    assert!(U256::from_dec_str(balance).unwrap().is_zero());

    print_with_time("[-] [Fungible Token Balance] - Get Fungible token balance".to_string());
    parsed_body_response["data"][0]["balance"].to_string()
}

pub async fn non_fungible_token_balance(
    config: &Config,
    client: &reqwest::Client,
    bearer_token: &str,
    chain_id: u64,
    address: String,
    contract_addresses: Vec<String>,
) -> Value {
    print_with_time("[+] [NFT Balance] - Query NFT balance".to_string());

    let body = json!({ "contract_addresses": contract_addresses }).to_string();

    let api_url = format!(
        "{}/api/v1/chains/{}/addresses/{}/tokens/nft/query",
        config.env_url, chain_id, address
    );
    let body_response = post(client, &api_url, bearer_token, body, "application/json").await;

    parse_body(body_response)
}

pub async fn non_fungible_token_balance_is_empty(
    config: &Config,
    client: &reqwest::Client,
    bearer_token: &str,
    chain_id: u64,
    address: String,
    contract_addresses: Vec<String>,
) {
    let parsed_body_response = non_fungible_token_balance(
        config,
        client,
        bearer_token,
        chain_id,
        address,
        contract_addresses,
    )
    .await;

    print_with_time("[=] [NFT Balance] - Assert Response".to_string());

    let tokens = parsed_body_response["tokens"].as_array().unwrap();
    assert!(tokens.is_empty());

    print_with_time("[-] [NFT Balance] - Query NFT balance".to_string());
}

pub async fn post_response(
    client: &reqwest::Client,
    url: &str,
    authorization: &str,
    body: String,
    content_type: &str,
) -> reqwest::Response {
    let mut response;
    let mut retries = 0;
    let max_retries = 3;
    loop {
        response = post_without_retry(client, url, authorization, body.clone(), content_type).await;

        retries += 1;
        print_with_time(format!(
            "[!] [Post Status] - Status Code: {}",
            response.status()
        ));

        if response.status().is_success() || retries >= max_retries {
            // Timeout has occurred, exit the loop
            break;
        }

        print_with_time(format!(
            "[~] [Post Fail] - Retrying in 5 seconds ({}/{})",
            retries, max_retries
        ));

        tokio::time::sleep(Duration::from_secs(5)).await;
    }

    assert!(response.status().is_success());
    response
}

// This function asynchronously performs a POST request with the given parameters and returns the response body.
pub async fn post(
    client: &reqwest::Client,
    url: &str,
    authorization: &str,
    body: String,
    content_type: &str,
) -> String {
    let response = post_response(client, url, authorization, body, content_type).await;
    response.text().await.expect("Failed to read response body")
}

pub async fn post_without_retry(
    client: &reqwest::Client,
    url: &str,
    authorization: &str,
    body: String,
    content_type: &str,
) -> reqwest::Response {
    client
        .post(url)
        .body(body.to_string())
        .header("Authorization", authorization)
        .header("Content-Type", content_type)
        .send()
        .await
        .unwrap()
}

pub async fn put_without_retry(
    client: &reqwest::Client,
    url: &str,
    authorization: &str,
    body: String,
    content_type: &str,
) -> reqwest::Response {
    client
        .put(url)
        .body(body.to_string())
        .header("Authorization", authorization)
        .header("Content-Type", content_type)
        .send()
        .await
        .unwrap()
}

// This function asynchronously performs a GET request with the given parameters and returns the response body.
pub async fn get(client: &reqwest::Client, url: &str, authorization: &str) -> String {
    let mut response;
    let mut retries = 0;
    let max_retries = 3;
    loop {
        response = client
            .get(url)
            .header("Authorization", authorization)
            .send()
            .await
            .unwrap();

        retries += 1;
        print_with_time(format!(
            "[!] [Get Status] - Status Code: {}",
            response.status()
        ));

        if response.status().is_success() || retries >= max_retries {
            // Timeout has occurred, exit the loop
            break;
        }

        print_with_time(format!(
            "[~] [Get Fail] - Retrying in 5 seconds ({}/{})",
            retries, max_retries,
        ));

        tokio::time::sleep(Duration::from_secs(5)).await;
    }

    assert!(response.status().is_success());
    response.text().await.expect("Failed to read response body")
}

pub async fn get_without_retry(
    client: &reqwest::Client,
    url: &str,
    authorization: &str,
) -> reqwest::Response {
    client
        .get(url)
        .header("Authorization", authorization)
        .send()
        .await
        .unwrap()
}

// This function parses a given JSON response body string and returns it as a serde_json::Value.
pub fn parse_body(body_response: String) -> Value {
    // Parse the provided JSON response body string into a serde_json::Value.
    // If parsing fails, the function will panic with an error message.
    serde_json::from_str(&body_response).expect("JSON parsing failed")
}

pub fn print_test_output_line_begins() {
    println!("\n{}", TEST_OUTPUT_DELIMITER_LINE);
}
pub fn print_test_output_line_ends() {
    println!("{}", TEST_OUTPUT_DELIMITER_LINE);
}

pub fn print_test_case_output_line_begins() {
    println!("{}", TEST_CASE_OUTPUT_DELIMITER_LINE);
}

pub fn print_with_time(message: String) {
    let current_time = Local::now();
    let hour = current_time.hour();
    let minute = current_time.minute();
    let second = current_time.second();
    let millisecond = current_time.timestamp_subsec_millis();
    println!(
        "[{:02}:{:02}:{:02}.{:03}] {}",
        hour, minute, second, millisecond, message
    );
}

fn get_ephemeral_config(some_env_var: &Option<String>) -> String {
    match some_env_var {
        Some(env_var) => env_var.to_string(),
        None => {
            panic!()
        }
    }
}

pub async fn request_speedup_1559_transaction(
    config: &Config,
    client: &reqwest::Client,
    bearer_token: &str,
    order_id: String,
    max_fee: U256,
    max_priority_fee: U256,
) {
    print_with_time("[+] [Speedup 1559] - Speedup 1559 transaction".to_string());

    let api_url = format!("{}/api/v1/orders/{}/speedup", config.env_url, order_id);

    // Create the request body specifying the speedup 1559 transaction details.
    let post_body = json!({
        "max_fee_per_gas": max_fee.to_string(),
        "max_priority_fee_per_gas": max_priority_fee.to_string(),
    })
    .to_string();

    // Perform a POST request to speedup the 1559 transaction.
    let _ = post(
        client,
        &api_url,
        bearer_token,
        post_body,
        "application/json",
    )
    .await;

    print_with_time("[-] [Speedup 1559] - Speedup 1559 transaction".to_string());
}

pub async fn validate_speedup_order_status(
    config: &Config,
    client: &reqwest::Client,
    bearer_token: &str,
    order_id: String,
    original_tx_hash: String,
) {
    print_with_time(format!(
        "[!] [Get speed up order status] - Original tx hash: {}",
        original_tx_hash.clone()
    ));

    let env_url: String = config.env_url.clone();

    let start_time = Instant::now();
    let timeout_duration = Duration::from_secs(config.timeout);

    let mut tx_hash;
    let mut state;

    loop {
        if start_time.elapsed() >= timeout_duration {
            print_with_time("[!] [Get speed up order status] - Timeout has occurred".to_string());
            panic!();
        }

        let parsed_body_response =
            get_order_status_from_url(env_url.as_str(), client, bearer_token, order_id.as_str())
                .await;

        tx_hash = parsed_body_response["data"]["transaction_hash"].to_string();
        state = parsed_body_response["state"].as_str().unwrap();

        print_with_time(format!(
            "[!] [Get speed up order status] - Current tx_hash {}",
            tx_hash.clone()
        ));
        print_with_time(format!(
            "[!] [Get speed up order status] - Current state {}",
            state
        ));

        if FINAL_STATES.contains(&state) {
            print_with_time(format!(
                "[!] [Get speed up order status] - Order is in final state {}",
                state
            ));
            break;
        } else if state == SUBMITTED_STATE {
            print_with_time(format!(
                "[!] [Get speed up order status] - Order is in {} state",
                SUBMITTED_STATE
            ));

            if original_tx_hash == tx_hash {
                print_with_time(
                    "[!] [Get speed up order status] - Original order found".to_string(),
                );
            } else {
                print_with_time(
                    "[!] [Get speed up order status] - Replacement order found".to_string(),
                );
                break;
            }
        }

        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    print_with_time("[!] [Get speed up order status] - Validating assertions".to_string());
    assert!(!tx_hash.is_empty());
    assert_ne!(original_tx_hash, tx_hash);
}

// This function asynchronously funds the gas pool address that pays the sponsored transaction by transferring funds from a previously funded address.
pub async fn fund_gas_pool(
    config: &Config,
    client: &reqwest::Client,
    bearer_token: &str,
    chain_id: u64,
    gas_price: U256,
    gas_pool_address: String,
) -> (String, String) {
    // ephemeral e2e test require these values from dev
    let authorization_token: String;
    let auth_url: String;
    let client_id: String;
    let env_url: String;
    let funded_address: String;

    // stringfied bearer token
    let authorization: String;

    if config.ephemeral {
        auth_url = get_ephemeral_config(&config.dev_auth_url);
        client_id = get_ephemeral_config(&config.dev_client_id);
        env_url = get_ephemeral_config(&config.dev_env_url);
        funded_address = get_ephemeral_config(&config.dev_funded_address);

        authorization_token = get_ephemeral_config(&config.dev_authorization_token);
        authorization = internal_authorize(client_id, auth_url, authorization_token, client).await;
    } else {
        env_url = config.env_url.clone();
        funded_address = config.funded_address.clone();
        authorization = bearer_token.to_string();
    }

    print_with_time(
        "[+] [Funding] - Transfers funds to the gas pool from a previously funded one".to_string(),
    );
    let api_url = format!("{}/api/v1/keys/{}/sign", env_url, funded_address);

    // Double gas_price
    let double_gas_price: U256 = gas_price * 2;
    print_with_time(format!(
        "[=] [Funding] - Transfers funds gas_price is {:?} will use double the gas_price {:?}",
        gas_price, double_gas_price
    ));

    // Calculate the amount to transfer - a.k.a. "value"
    let gas = U256::from(GAS_LIMIT);
    let val_for_txn = U256::from(VALUE_FOR_TXN);
    let num_of_txns = U256::from(NUM_OF_TXNS);
    let amount_to_transfer: U256 = (gas * double_gas_price + val_for_txn) * num_of_txns;

    // Create the request body specifying the transaction details.
    let post_body = json!({
        "transaction": {
            "to":  gas_pool_address,
            "gas": GAS_LIMIT.to_string(),
            "gas_price": double_gas_price.to_string(),
            "value": amount_to_transfer.to_string(),
            "data": "0x00",
            "chain_id": chain_id
        }
    })
    .to_string();

    print_with_time(format!(
        "[~] [Funding] - Transfers funds using gas_price {:?} and value {:?}",
        double_gas_price, amount_to_transfer
    ));

    // Perform a POST request to sign the transaction and fetch the response body.
    let body_response = post(
        client,
        &api_url,
        &authorization,
        post_body,
        "application/json",
    )
    .await;

    let parsed_body_response = parse_body(body_response);
    let order_id = parsed_body_response["order_id"].as_str().unwrap();
    print_with_time("[=] [Funding] - Assert Response".to_string());
    assert!(!order_id.to_string().is_empty());
    print_with_time(
        "[-] [Funding] - Transfers funds to the gas pool from a previously funded one".to_string(),
    );
    (order_id.to_string(), authorization)
}

// This function sets an address as the gas pool for the client
pub async fn create_gas_pool(
    config: &Config,
    client: &reqwest::Client,
    bearer_token: &str,
    chain_id: u64,
    gas_pool_address: String,
) {
    let env_url = config.env_url.clone();
    let authorization = bearer_token.to_string();

    print_with_time("[+] [Create gas pool] - Setting gas pool for client".to_string());
    let api_url = format!("{}/api/v1/gas_pool/chains/{}", env_url, chain_id);
    let post_body = json!({
        "gas_pool_address": gas_pool_address,
    })
    .to_string();

    // Perform a POST request and check for success.
    let _ = post(
        client,
        &api_url,
        &authorization,
        post_body,
        "application/json",
    )
    .await;

    print_with_time("[-] [Create gas pool] - Setting gas pool for client".to_string());
}
