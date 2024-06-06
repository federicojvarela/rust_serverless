mod dtos;

use crate::dtos::TransactionLogEvent;
use async_trait::async_trait;
use dtos::OrderStateFromTxLog;
use model::order::OrderState;
use mpc_signature_sm::blockchain::event_log::EventLog;
use mpc_signature_sm::result::error::OrchestrationError;
use mpc_signature_sm::{
    lambda_main, lambda_structure::lambda_trait::Lambda, result::error::Result,
};

pub struct Persisted {}

pub struct MpcComputeOrderStateFromTxLog;

#[async_trait]
impl Lambda for MpcComputeOrderStateFromTxLog {
    type PersistedMemory = Persisted;
    type InputBody = TransactionLogEvent;
    type Output = OrderStateFromTxLog;
    type Error = OrchestrationError;

    async fn bootstrap() -> Result<Self::PersistedMemory> {
        Ok(Persisted {})
    }

    async fn run(request: Self::InputBody, _state: &Self::PersistedMemory) -> Result<Self::Output> {
        let log = request.detail;
        let result = EventLog::decode_event_log(log)
            .map_err(|e| OrchestrationError::unknown(format!("Error decoding event log: {e}")))?;

        let order_state = match result.event_success {
            Some(true) => OrderState::Completed.to_string(),
            Some(false) => OrderState::CompletedWithError.to_string(),
            None => "".to_owned(),
        };

        let response = OrderStateFromTxLog {
            order_state,
            event_name: result.event_name,
            event_signature: result.event_signature,
        };
        Ok(response)
    }
}

lambda_main!(MpcComputeOrderStateFromTxLog);

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use crate::{dtos::TransactionLogEvent, MpcComputeOrderStateFromTxLog, Persisted};
    use ethers::abi::AbiEncode;
    use ethers::types::{Bytes, Log, H256};
    use mpc_signature_sm::blockchain::event_log::{
        ERC2771_FORWARDER_EXPIRED_REQUEST, EXECUTED_FORWARD_REQUEST,
    };
    use mpc_signature_sm::lambda_structure::lambda_trait::Lambda;
    use rstest::*;

    const DATA_SUCCESS: &str = "0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001";
    const DATA_ERROR: &str = "0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000";

    struct TestFixture {}

    #[fixture]
    fn fixture() -> TestFixture {
        TestFixture {}
    }

    fn build_input(signature: &str, data: &str) -> TransactionLogEvent {
        let mut log = Log::default();
        let signature = H256::from_str(signature).unwrap();
        log.topics = vec![signature];
        let data = Bytes::from_str(data).unwrap();
        log.data = data;

        TransactionLogEvent { detail: log }
    }

    fn build_input_no_signature() -> TransactionLogEvent {
        TransactionLogEvent {
            detail: Log::default(),
        }
    }

    #[rstest]
    #[tokio::test]
    async fn compute_order_state_empty_topics(mut _fixture: TestFixture) {
        let request = build_input_no_signature();

        MpcComputeOrderStateFromTxLog::run(request, &Persisted {})
            .await
            .unwrap_err();
    }

    #[rstest]
    #[tokio::test]
    async fn compute_order_state_successful(mut _fixture: TestFixture) {
        let request = build_input(EXECUTED_FORWARD_REQUEST, DATA_SUCCESS);

        let order_state = MpcComputeOrderStateFromTxLog::run(request, &Persisted {})
            .await
            .unwrap();

        assert_eq!(order_state.order_state, "COMPLETED");
    }

    #[rstest]
    #[tokio::test]
    async fn compute_order_state_data_error(mut _fixture: TestFixture) {
        let request = build_input(EXECUTED_FORWARD_REQUEST, DATA_ERROR);

        let order_state = MpcComputeOrderStateFromTxLog::run(request, &Persisted {})
            .await
            .unwrap();

        assert_eq!(order_state.order_state, "COMPLETED_WITH_ERROR");
    }

    #[rstest]
    #[tokio::test]
    async fn compute_order_state_erc2771_error(mut _fixture: TestFixture) {
        let request = build_input(ERC2771_FORWARDER_EXPIRED_REQUEST, DATA_SUCCESS);

        let order_state = MpcComputeOrderStateFromTxLog::run(request, &Persisted {})
            .await
            .unwrap();

        assert_eq!(order_state.order_state, "COMPLETED_WITH_ERROR");
    }

    #[rstest]
    #[tokio::test]
    async fn compute_order_state_unhandled_error(mut _fixture: TestFixture) {
        let unhandled_signature = H256::random().encode_hex();
        let request = build_input(unhandled_signature.as_str(), DATA_SUCCESS);

        let order_state = MpcComputeOrderStateFromTxLog::run(request, &Persisted {})
            .await
            .unwrap();

        assert_eq!(order_state.order_state, "");
    }
}
