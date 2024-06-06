use crate::fixtures::lambda::{fixture, LambdaFixture};
use crate::helpers::lambda::LambdaResponse;
use ana_tools::config_loader::ConfigLoader;
use ethers::abi::AbiEncode;
use ethers::types::H256;
use http::StatusCode;
use rstest::*;
use serde::Deserialize;
use serde_json::{json, Value};

const FUNCTION_NAME: &str = "mpc_compute_order_state_from_tx_log";

const EXECUTED_FORWARD_REQUEST: &str =
    "0x842fb24a83793558587a3dab2be7674da4a51d09c5542d6dd354e5d0ea70813c";
const ERC2771_FORWARD_INVALID_SIGNER: &str =
    "0xc845a056973bc1f7f2d7cd71736668c2145d9639779c36b557dd323c0d18f784";
const DATA_SUCCESS: &str = "0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001";
const DATA_ERROR: &str = "0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000";

#[derive(Deserialize, Debug)]
pub struct OrderStateFromTxLog {
    pub order_state: String,
    pub event_name: Option<String>,
    pub event_signature: Option<String>,
}

#[derive(Deserialize)]
pub struct Config {}

pub struct LocalFixture {
    pub config: Config,
}

#[fixture]
async fn local_fixture() -> LocalFixture {
    let config = ConfigLoader::load_test::<Config>();

    LocalFixture { config }
}

fn get_event_log(signature: &str, data: &str) -> Value {
    json!(
    {
      "version": "0",
      "id": "3b7bafc1-cb41-5613-8801-5bad0d3d6af2",
      "detail-type": "publish_tx_log_event",
      "source": "ana-chain-listener-polygon-80002-tx-log",
      "account": "267505102317",
      "time": "2024-03-27T13:10:34Z",
      "region": "us-west-2",
      "resources": [],
      "detail": {
        "address": "0xbf9a136b22c951924e7b4b169bcbd79c27b85f08",
        "topics": [
          signature,
          "0x0000000000000000000000009d1fc1cfd0a23d7bee6d38ba65c782352ab51c51"
        ],
        "data": data,
        "blockHash": "0x2253c3dd5c7080917da34bd94d5fc8c1de280a0e8d464333cfa373ce4cde0907",
        "blockNumber": "0x2d579e6",
        "transactionHash": "0x8613a0eb4460453ad33c0d84135d080a2b682b8fbe323cd3212120b990ebc4bd",
        "transactionIndex": "0x0",
        "logIndex": "0x0",
        "removed": false
      }
    })
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn mpc_compute_order_state_from_tx_log_successful(
    fixture: &LambdaFixture,
    #[future] local_fixture: LocalFixture,
) {
    let _local_fixture = local_fixture.await;

    let input = get_event_log(EXECUTED_FORWARD_REQUEST, DATA_SUCCESS);

    let response: LambdaResponse<OrderStateFromTxLog> = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|_| panic!("There was an error invoking {FUNCTION_NAME}"));

    assert_eq!(StatusCode::OK, response.status);
    assert_eq!("COMPLETED", response.body.order_state);
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn mpc_compute_order_state_from_tx_log_data_error(
    fixture: &LambdaFixture,
    #[future] local_fixture: LocalFixture,
) {
    let _local_fixture = local_fixture.await;

    let input = get_event_log(EXECUTED_FORWARD_REQUEST, DATA_ERROR);

    let response: LambdaResponse<OrderStateFromTxLog> = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|_| panic!("There was an error invoking {FUNCTION_NAME}"));

    assert_eq!(StatusCode::OK, response.status);
    assert_eq!("COMPLETED_WITH_ERROR", response.body.order_state);
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn mpc_compute_order_state_from_tx_log_unsuccessful(
    fixture: &LambdaFixture,
    #[future] local_fixture: LocalFixture,
) {
    let _local_fixture = local_fixture.await;

    let input = get_event_log(ERC2771_FORWARD_INVALID_SIGNER, DATA_ERROR);

    let response: LambdaResponse<OrderStateFromTxLog> = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|_| panic!("There was an error invoking {FUNCTION_NAME}"));

    assert_eq!(StatusCode::OK, response.status);
    assert_eq!("COMPLETED_WITH_ERROR", response.body.order_state);
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn mpc_compute_order_state_from_tx_log_unhandled_error(
    fixture: &LambdaFixture,
    #[future] local_fixture: LocalFixture,
) {
    let _local_fixture = local_fixture.await;

    let unhandled_signature = H256::random().encode_hex();
    let input = get_event_log(unhandled_signature.as_str(), DATA_SUCCESS);

    let response: LambdaResponse<OrderStateFromTxLog> = fixture
        .lambda
        .invoke(FUNCTION_NAME, input)
        .await
        .unwrap_or_else(|_| panic!("There was an error invoking {FUNCTION_NAME}"));

    assert_eq!(StatusCode::OK, response.status);
    assert_eq!("", response.body.order_state);
}
