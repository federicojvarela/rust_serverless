use std::str::FromStr;

use crate::fixtures::lambda::{fixture, LambdaFixture};
use crate::fixtures::provider::{provider_fixture, ProviderFixture};
use crate::helpers::lambda::LambdaResponse;

use common::test_tools::http::constants::{
    ADDRESS_FOR_MOCK_REQUESTS, KEY_ID_FOR_MOCK_REQUESTS, ORDER_ID_FOR_MOCK_REQUESTS,
};
use ethers::providers::{Http, Middleware, Provider};
use ethers::signers::{LocalWallet, Signer};
use ethers::types::transaction::eip2718::TypedTransaction;
use ethers::types::{TransactionRequest, TxHash};
use ethers::utils::parse_ether;
use http::StatusCode;
use mpc_signature_sm::http::utils::SUBMISSION_ERROR_CODE;
use rstest::rstest;
use serde_json::{json, Value};
use tokio::time::{sleep, timeout, Duration};

const FUNCTION_NAME: &str = "send_transaction";

fn get_lambda_input(hex_rlp: String) -> Value {
    json!({
        "payload": {
            "transaction_hash": "0x5ab92ed04f3dbc8dd3c9d7930e83245fae4398685cafc757195b27a6940c78ba",
            "maestro_signature": hex_rlp,
            "transaction": {
                "data": "0x6406516041610651325106165165106516169610",
                "gas": "300000",
                "gas_price": "300000000",
                "to": ADDRESS_FOR_MOCK_REQUESTS.to_string(),
                "value": "111111",
                "nonce": "15",
                "chain_id": 1
            },
            "approval_status": "APPROVED",
            "key_id": KEY_ID_FOR_MOCK_REQUESTS
        },
        "context": {
            "order_id": ORDER_ID_FOR_MOCK_REQUESTS
        }
    })
}

async fn get_hex_rlp(provider: &Provider<Http>, amount_of_gas: u64) -> String {
    let accounts = provider.get_accounts().await.unwrap();
    assert!(accounts.len() >= 2, "Expected at least two accounts");

    let wallet = "0xd3451c75d4e764a197d9a0fef918763b858cd5aa228df299fd4627042429d29a"
        .to_string()
        .parse::<LocalWallet>()
        .unwrap()
        .with_chain_id(1337_u64);

    let tx: TypedTransaction = TransactionRequest::new()
        .from(accounts[0])
        .to(accounts[1])
        .value(parse_ether("1").unwrap())
        .gas(amount_of_gas)
        .gas_price(900000000)
        .chain_id(1337)
        .into();

    let signature = wallet.sign_transaction(&tx).await.unwrap();
    hex::encode(tx.rlp_signed(&signature))
}

#[rstest]
#[tokio::test]
pub async fn send_transaction_ok(fixture: &LambdaFixture, provider_fixture: &ProviderFixture) {
    let provider = &provider_fixture.provider;
    let hex_rlp = get_hex_rlp(provider, 300000).await;
    let input = get_lambda_input(hex_rlp);

    let response: LambdaResponse<Value> = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|e| panic!("There was an error invoking {FUNCTION_NAME}: {e:?}"));

    assert_eq!(StatusCode::OK, response.status);

    let payload = response.body.get("payload").unwrap();
    let tx_hash = payload["Submitted"]["tx_hash"].as_str().unwrap();
    let tx_hash = TxHash::from_str(tx_hash).unwrap();

    // Continuously check for the transaction with a timeout of 60s
    let receipt = timeout(Duration::from_secs(60), async {
        loop {
            match provider.get_transaction_receipt(tx_hash).await.unwrap() {
                Some(receipt) => break receipt,
                None => sleep(Duration::from_secs(1)).await,
            };
        }
    })
    .await
    .unwrap();

    assert!(receipt.status.unwrap() == 1.into(), "Transaction failed");
}

#[rstest]
#[tokio::test]
pub async fn send_transaction_submission_error(
    fixture: &LambdaFixture,
    provider_fixture: &ProviderFixture,
) {
    let provider = &provider_fixture.provider;
    let hex_rlp = get_hex_rlp(provider, 0).await;
    let input = get_lambda_input(hex_rlp);

    let response: LambdaResponse<Value> = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|e| panic!("There was an error invoking {FUNCTION_NAME}: {e:?}"));

    assert_eq!(StatusCode::OK, response.status);

    let payload = response.body.get("payload").unwrap();
    let code = payload["NotSubmitted"]["code"].as_i64().unwrap();
    assert_eq!(SUBMISSION_ERROR_CODE, code);
}
