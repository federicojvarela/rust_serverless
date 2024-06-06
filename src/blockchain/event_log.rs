use anyhow::anyhow;
use ethers::{abi::AbiEncode, types::Log};
use serde::{Deserialize, Serialize};

pub const EXECUTED_FORWARD_REQUEST: &str =
    "0x842fb24a83793558587a3dab2be7674da4a51d09c5542d6dd354e5d0ea70813c";
pub const ERC2771_FORWARD_INVALID_SIGNER: &str =
    "0xc845a056973bc1f7f2d7cd71736668c2145d9639779c36b557dd323c0d18f784";
pub const ERC2771_FORWARDER_MISMATCHED_VALUE: &str =
    "0x70647f79f9d7612ec5cfa541f407ca826be01b69a9a7b3e583781b1002fd93c7";
pub const ERC2771_FORWARDER_EXPIRED_REQUEST: &str =
    "0x94eef58a33b817a1b65237e0f9d0e329b852d5ae15f050799b8441eae4390556";
pub const ERC2771_UNTRUSTFUL_TARGET: &str =
    "0xd2650cd17abcf9f73bc10fd31970fbe854729f4bab904be0d9865a7e3773aa63";

#[derive(Deserialize, Debug, Serialize, PartialEq, Eq, Clone)]
pub struct EventLogResponse {
    pub event_name: Option<String>,
    pub event_signature: Option<String>,
    pub event_success: Option<bool>,
}

#[derive(Deserialize)]
pub struct EventLog {}

impl EventLog {
    pub fn decode_event_log(event_log: Log) -> Result<EventLogResponse, anyhow::Error> {
        if event_log.topics.is_empty() {
            return Err(anyhow!("Topics list was empty"));
        }

        // Get event signature from log
        let event_signature = event_log.topics[0].encode_hex();

        match event_signature.as_str() {
            EXECUTED_FORWARD_REQUEST => {
                // Get last item from data > Status from this contract
                // TODO: This is highly coupled to the contract definition (Sponsored TXs)
                // Need to find a better way to address this need
                let event_success = matches!(event_log.data.last(), Some(&1));

                Ok(EventLogResponse {
                    event_name: Some("EXECUTED_FORWARD_REQUEST".to_string()),
                    event_signature: Some(event_signature),
                    event_success: Some(event_success),
                })
            }
            ERC2771_FORWARD_INVALID_SIGNER => Ok(EventLogResponse {
                event_name: Some("ERC2771_FORWARD_INVALID_SIGNER".to_string()),
                event_signature: Some(event_signature),
                event_success: Some(false),
            }),
            ERC2771_FORWARDER_MISMATCHED_VALUE => Ok(EventLogResponse {
                event_name: Some("ERC2771_FORWARDER_MISMATCHED_VALUE".to_string()),
                event_signature: Some(event_signature),
                event_success: Some(false),
            }),
            ERC2771_FORWARDER_EXPIRED_REQUEST => Ok(EventLogResponse {
                event_name: Some("ERC2771_FORWARDER_EXPIRED_REQUEST".to_string()),
                event_signature: Some(event_signature),
                event_success: Some(false),
            }),
            ERC2771_UNTRUSTFUL_TARGET => Ok(EventLogResponse {
                event_name: Some("ERC2771_UNTRUSTFUL_TARGET".to_string()),
                event_signature: Some(event_signature),
                event_success: Some(false),
            }),
            _ => Ok(EventLogResponse {
                event_name: None,
                event_signature: Some(event_signature),
                event_success: None,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::blockchain::event_log::{
        EventLog, ERC2771_FORWARDER_EXPIRED_REQUEST, ERC2771_FORWARDER_MISMATCHED_VALUE,
        ERC2771_FORWARD_INVALID_SIGNER, ERC2771_UNTRUSTFUL_TARGET, EXECUTED_FORWARD_REQUEST,
    };
    use ethers::types::Log;
    use rstest::*;
    use serde_json::json;

    const UNKNOWN_EVENT_SIGNATURE: &str =
        "0x4dfe1bbbcf077ddc3e01291eea2d5c70c2b422b415d95645b9adcfd678cb1d63";

    fn build_log(topic: String, data: String) -> Log {
        serde_json::from_value::<Log>(json!({
            "address": "0xc59f67a8bff5d8cd03f6ac17265c550ed8f33907",
            "blockHash": "0x8243343df08b9751f5ca0c5f8c9c0460d8a9b6351066fae0acbd4d3e776de8bb",
            "blockNumber": "0x429d3b",
            "data": data,
            "logIndex": "0x56",
            "removed": false,
            "topics": [
                topic,
                "0x000000000000000000000000c0ccbc1f4596c7dd07f42fe2f0d3304aa97c9ed6"
            ],
            "transactionHash": "0xab059a62e22e230fe0f56d8555340a29b2e9532360368f810595453f6fdd213b",
            "transactionIndex": "0xac"
        }))
        .unwrap()
    }

    const DATA_SUCCESS: &str = "0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001";
    const DATA_ERROR: &str = "0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000";

    #[rstest]
    #[tokio::test(flavor = "multi_thread")]
    async fn get_successful_event_signature() {
        let log = build_log(
            EXECUTED_FORWARD_REQUEST.to_string(),
            DATA_SUCCESS.to_string(),
        );
        let result = EventLog::decode_event_log(log.clone()).unwrap();

        assert_eq!(
            result.event_name,
            Some("EXECUTED_FORWARD_REQUEST".to_string())
        );
        assert_eq!(
            result.event_signature,
            Some(EXECUTED_FORWARD_REQUEST.to_string())
        );
        assert_eq!(result.event_success, Some(true));
    }

    #[rstest]
    #[tokio::test(flavor = "multi_thread")]
    async fn get_error_event_signature() {
        let log = build_log(EXECUTED_FORWARD_REQUEST.to_string(), DATA_ERROR.to_string());
        let result = EventLog::decode_event_log(log.clone()).unwrap();

        assert_eq!(
            result.event_name,
            Some("EXECUTED_FORWARD_REQUEST".to_string())
        );
        assert_eq!(
            result.event_signature,
            Some(EXECUTED_FORWARD_REQUEST.to_string())
        );
        assert_eq!(result.event_success, Some(false));
    }

    #[rstest]
    #[tokio::test(flavor = "multi_thread")]
    async fn get_invalid_signed_error() {
        let log = build_log(
            ERC2771_FORWARD_INVALID_SIGNER.to_string(),
            DATA_ERROR.to_string(),
        );
        let result = EventLog::decode_event_log(log.clone()).unwrap();

        assert_eq!(
            result.event_name,
            Some("ERC2771_FORWARD_INVALID_SIGNER".to_string())
        );
        assert_eq!(
            result.event_signature,
            Some(ERC2771_FORWARD_INVALID_SIGNER.to_string())
        );
        assert_eq!(result.event_success, Some(false));
    }

    #[rstest]
    #[tokio::test(flavor = "multi_thread")]
    async fn get_mismatched_value_error() {
        let log = build_log(
            ERC2771_FORWARDER_MISMATCHED_VALUE.to_string(),
            DATA_ERROR.to_string(),
        );
        let result = EventLog::decode_event_log(log.clone()).unwrap();

        assert_eq!(
            result.event_name,
            Some("ERC2771_FORWARDER_MISMATCHED_VALUE".to_string())
        );
        assert_eq!(
            result.event_signature,
            Some(ERC2771_FORWARDER_MISMATCHED_VALUE.to_string())
        );
        assert_eq!(result.event_success, Some(false));
    }

    #[rstest]
    #[tokio::test(flavor = "multi_thread")]
    async fn get_expired_request_error() {
        let log = build_log(
            ERC2771_FORWARDER_EXPIRED_REQUEST.to_string(),
            DATA_ERROR.to_string(),
        );
        let result = EventLog::decode_event_log(log.clone()).unwrap();

        assert_eq!(
            result.event_name,
            Some("ERC2771_FORWARDER_EXPIRED_REQUEST".to_string())
        );
        assert_eq!(
            result.event_signature,
            Some(ERC2771_FORWARDER_EXPIRED_REQUEST.to_string())
        );
        assert_eq!(result.event_success, Some(false));
    }

    #[rstest]
    #[tokio::test(flavor = "multi_thread")]
    async fn get_untrustful_target_error() {
        let log = build_log(
            ERC2771_UNTRUSTFUL_TARGET.to_string(),
            DATA_ERROR.to_string(),
        );
        let result = EventLog::decode_event_log(log.clone()).unwrap();

        assert_eq!(
            result.event_name,
            Some("ERC2771_UNTRUSTFUL_TARGET".to_string())
        );
        assert_eq!(
            result.event_signature,
            Some(ERC2771_UNTRUSTFUL_TARGET.to_string())
        );
        assert_eq!(result.event_success, Some(false));
    }

    #[rstest]
    #[tokio::test(flavor = "multi_thread")]
    async fn get_unknown_signature_error() {
        let log = build_log(UNKNOWN_EVENT_SIGNATURE.to_string(), DATA_ERROR.to_string());
        let result = EventLog::decode_event_log(log.clone()).unwrap();

        assert_eq!(
            result.event_signature,
            Some(UNKNOWN_EVENT_SIGNATURE.to_string())
        );
        assert_eq!(result.event_success, None);
    }
}
