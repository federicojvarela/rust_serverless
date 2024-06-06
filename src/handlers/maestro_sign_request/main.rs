mod dtos;
mod models;

use crate::models::MaestroSignResponse;
use async_trait::async_trait;
use common::aws_clients::secrets_manager::get_secrets_provider;
use dtos::{
    requests::{MaestroSignatureRequest, SignatureRequest},
    responses::SignatureResponse,
};
use mpc_signature_sm::{
    lambda_main,
    lambda_structure::{event::Event, lambda_trait::Lambda},
    maestro::{maestro_bootstrap, state::MaestroState},
    result::{error::OrchestrationError, Result},
};
use reqwest::StatusCode;

const REJECTION_BODY: &str = "Metadata approval status is not equals to one.";

pub struct MaestroSignRequest;

#[async_trait]
impl Lambda for MaestroSignRequest {
    type PersistedMemory = MaestroState;
    type InputBody = Event<SignatureRequest>;
    type Output = Event<SignatureResponse>;
    type Error = OrchestrationError;

    async fn bootstrap() -> Result<Self::PersistedMemory> {
        let secrets_provider = get_secrets_provider().await;
        maestro_bootstrap(secrets_provider).await
    }

    async fn run(request: Self::InputBody, state: &Self::PersistedMemory) -> Result<Self::Output> {
        let maestro_signature_request: MaestroSignatureRequest =
            request.payload.clone().try_into()?;
        let response = state
            .http
            .post(format!("{}/sign", &state.config.maestro_url))
            .json(&maestro_signature_request)
            .send()
            .await?;

        let status = response.status();
        let text = response.text().await?;

        // Act
        let response = parse_response(status, text).await?;

        let transaction_with_correct_nonce = request
            .payload
            .transaction
            .into_transaction_request_with_nonce(request.payload.replacement_nonce);

        Ok(Event {
            payload: SignatureResponse {
                approval: response.into(),
                transaction: transaction_with_correct_nonce.into(),
                key_id: request.payload.key_id,
            },
            context: request.context,
        })
    }
}

async fn parse_response(status: StatusCode, text: String) -> Result<MaestroSignResponse> {
    let unprocessable_err = OrchestrationError::unknown(format!(
        "Maestro returned an unprocessable content result. Status: {status}, Response Text: {text}"
    ));

    let unknown_err = OrchestrationError::unknown(format!(
        "Maestro returned an unexpected result. Status: {status}, Response Text: {text}"
    ));

    match status {
        StatusCode::OK =>
        // HAPPY_PATH
        {
            serde_json::from_str(&text).map_err(|_| unprocessable_err)
        }

        StatusCode::UNPROCESSABLE_ENTITY => {
            if text == REJECTION_BODY {
                Ok(MaestroSignResponse::Rejected {
                    reason: REJECTION_BODY.to_string(),
                })
            } else {
                Err(unprocessable_err)
            }
        }
        _ => Err(unknown_err),
    }
}

lambda_main!(MaestroSignRequest);

#[cfg(test)]
mod tests {
    const SIGNATURE: &str = "f87e833135318411e1a300830493e0941c965d1241d0040a3fc2a030baeeefb35c155a428301b20794640651604161065132510616516510651616961025a013c461209501269b322e946ad3b0b8a899e11d1cfd30d0b45944cf40cac78f76a06bef34da10ff055f3edb8201a420b77c4adc9d51f92330bc12d9909f1a87b54b";
    const TRANSACTION_HASH: &str =
        "26d47c86afe7482ab77835290c03ee428eb281d10cf5d479cb10636941917ef8";
    const RPL_ENCODED_SIGNED_TRANSACTION: &str = r#"{"rlp_encoded_signed_transaction":"f87e833135318411e1a300830493e0941c965d1241d0040a3fc2a030baeeefb35c155a428301b20794640651604161065132510616516510651616961025a013c461209501269b322e946ad3b0b8a899e11d1cfd30d0b45944cf40cac78f76a06bef34da10ff055f3edb8201a420b77c4adc9d51f92330bc12d9909f1a87b54b","transaction_hash":"26d47c86afe7482ab77835290c03ee428eb281d10cf5d479cb10636941917ef8"}"#;
    const RPL_ENCODED_BAD_SIGNED_TRANSACTION: &str = r#"{"rlp_encoded_signed_transaction":"f87e833135318411e1a300830493e0941c965d1241d0040a3fc2a030baeeefb35c155a428301b20794640651604161065132510616516510651616961025a013c461209501269b322e946ad3b0b8a899e11d1cfd30d0b45944cf40cac78f76a06bef34da10ff055f3edb8201a420b77c4adc9d51f92330bc12d9909f1a87b54b","transaction_hash":"26d47c86afe7482ab77835290c03ee428eb281d10cf5d47"}"#;

    use super::*;
    use crate::dtos::requests::SignatureRequest;
    use core::panic;
    use ethers::types::{H256, U256};
    use model::order::policy::Policy;
    use mpc_signature_sm::dtos::requests::transaction_request::TransactionRequestNoNonce;
    use mpc_signature_sm::{
        maestro::{
            config::MaestroConfig,
            session::{login, MaestroLoginInformation},
        },
        rest::middlewares::AuthenticationMiddleware,
        result::error::OrchestrationError,
    };
    use rstest::*;
    use serde_json::json;
    use serde_json::Value;
    use std::sync::Arc;
    use uuid::Uuid;
    use wiremock::matchers::body_partial_json;
    use wiremock::{
        matchers::{method, path},
        Mock, MockServer, ResponseTemplate,
    };

    struct TestFixture {
        pub state: MaestroState,
        pub mock_server: MockServer,
    }

    #[fixture]
    async fn fixture() -> TestFixture {
        let mock_server = MockServer::start().await;
        let config = MaestroConfig {
            maestro_url: mock_server.uri(),
            service_name: "test".to_owned(),
            maestro_api_key_secret_name: "dummy_secret_name_api_key".to_owned(),
            maestro_tenant_name: "tenant".to_owned(),
        };

        let http_client = reqwest_middleware::ClientBuilder::new(reqwest::Client::new())
            .with(AuthenticationMiddleware::new(
                &login,
                Arc::new(MaestroLoginInformation {
                    maestro_url: config.maestro_url.clone(),
                    service_name: config.service_name.clone(),
                    maestro_api_key: "dummy_api_secret".to_owned(),
                    tenant_name: "tenant".to_owned(),
                }),
                Some("dummy_token".to_owned()),
            ))
            .build();

        TestFixture {
            state: MaestroState {
                http: http_client,
                config,
            },
            mock_server,
        }
    }

    fn get_transaction_request(
        gas_price: Option<U256>,
        max_priority_fee_per_gas: Option<U256>,
        max_fee_per_gas: Option<U256>,
    ) -> TransactionRequestNoNonce {
        if let Some(gp) = gas_price {
            TransactionRequestNoNonce::Legacy {
                to: b"0x1c965d1241d0040a3f".into(),
                gas: 30000000.into(),
                gas_price: gp,
                value: 100.into(),
                data: hex::decode("6406516041610651325106165165106516169610")
                    .unwrap()
                    .into(),
                chain_id: 1,
            }
        } else if let (Some(mpf), Some(mf)) = (max_priority_fee_per_gas, max_fee_per_gas) {
            TransactionRequestNoNonce::Eip1559 {
                to: b"0x1c965d1241d0040a3f".into(),
                gas: 30000000.into(),
                max_priority_fee_per_gas: mpf,
                max_fee_per_gas: mf,
                value: 100.into(),
                data: hex::decode("6406516041610651325106165165106516169610")
                    .unwrap()
                    .into(),
                chain_id: 1,
            }
        } else {
            panic!("invalid transaction configuration");
        }
    }

    fn get_correct_signature_request(
        gas_price: Option<U256>,
        max_priority_fee_per_gas: Option<U256>,
        max_fee_per_gas: Option<U256>,
    ) -> Event<SignatureRequest> {
        let transaction =
            get_transaction_request(gas_price, max_priority_fee_per_gas, max_fee_per_gas);
        Event::test_event_from(SignatureRequest {
            transaction,
            key_id: Uuid::parse_str("4cbdc9a0-b3dd-47d1-a7e3-eabe93f1118c").unwrap(),
            replacement_nonce: 15.into(),
            policy: Policy {
                name: String::default(),
                approvals: vec![],
            },
        })
    }

    fn get_rejection_signature_request(
        gas_price: Option<U256>,
        max_priority_fee_per_gas: Option<U256>,
        max_fee_per_gas: Option<U256>,
    ) -> Event<SignatureRequest> {
        let transaction =
            get_transaction_request(gas_price, max_priority_fee_per_gas, max_fee_per_gas);
        Event::test_event_from(SignatureRequest {
            transaction,
            key_id: Uuid::parse_str("4cbdc9a0-b3dd-47d1-a7e3-eabe93f1118c").unwrap(),
            replacement_nonce: 15.into(),
            policy: Policy {
                name: String::default(),
                approvals: vec![],
            },
        })
    }

    #[rstest]
    #[case::legacy_transaction(Some(800000.into()), None, None)]
    #[case::eip_1559_compatible_tx(None, Some(U256::from(300000)), Some(U256::from(300000)))]
    #[tokio::test]
    async fn sign_transaction_approved(
        #[future] fixture: TestFixture,
        #[case] gas_price: Option<U256>,
        #[case] max_priority_fee_per_gas: Option<U256>,
        #[case] max_fee_per_gas: Option<U256>,
    ) {
        //required field
        let expected_body = json!({
            "policies": []
        });

        // Arrange
        let fixture = fixture.await;
        Mock::given(method("POST"))
            .and(path("/sign"))
            .and(body_partial_json(expected_body))
            .respond_with(ResponseTemplate::new(StatusCode::OK).set_body_json(json!({
                "rlp_encoded_signed_transaction": SIGNATURE,
                "transaction_hash": TRANSACTION_HASH,
            })))
            .expect(1)
            .mount(&fixture.mock_server)
            .await;

        let request =
            get_correct_signature_request(gas_price, max_priority_fee_per_gas, max_fee_per_gas);

        // Act
        let response = MaestroSignRequest::run(request, &fixture.state).await;

        // Assert
        let response = response.unwrap();
        let (maestro_signature, _) = response.payload.approval.as_result().unwrap();

        assert_eq!(maestro_signature, SIGNATURE);
    }

    #[rstest]
    #[tokio::test]
    async fn sign_transaction_maestro_200_unknown_body(#[future] fixture: TestFixture) {
        //required field
        let expected_body = json!({
            "policies": []
        });

        // Arrange
        let fixture = fixture.await;
        Mock::given(method("POST"))
            .and(path("/sign"))
            .and(body_partial_json(expected_body))
            .respond_with(ResponseTemplate::new(StatusCode::OK).set_body_json(json!({
                "signature": "dummy",
                "order_id": "dummy",
            })))
            .expect(1)
            .mount(&fixture.mock_server)
            .await;

        let request = get_correct_signature_request(Some(800000.into()), None, None);

        // Act
        let response = MaestroSignRequest::run(request, &fixture.state).await;

        // Assert
        assert!(response.is_err());
        let response = response.unwrap_err();
        assert!(matches!(response, OrchestrationError::Unknown(_)));
    }

    #[rstest]
    #[case(500)]
    #[case(422)]
    #[case(400)]
    #[tokio::test]
    async fn sign_transaction_return_unknown_if_maestro_fails(
        #[case] http_status: u16,
        #[future] fixture: TestFixture,
    ) {
        let expected_body = json!({
            "policies": []
        });

        // Arrange
        let fixture = fixture.await;
        Mock::given(method("POST"))
            .and(path("/sign"))
            .and(body_partial_json(expected_body))
            .respond_with(ResponseTemplate::new(http_status))
            .expect(1)
            .mount(&fixture.mock_server)
            .await;

        let request = get_correct_signature_request(Some(800000.into()), None, None);

        // Act
        let response = MaestroSignRequest::run(request, &fixture.state).await;

        // Assert
        assert!(response.is_err());
        let response = response.unwrap_err();
        assert!(matches!(response, OrchestrationError::Unknown(_)));
    }

    #[rstest]
    #[tokio::test]
    async fn handle_rejected_error(#[future] fixture: TestFixture) {
        let expected_body = json!({
            "policies": []
        });

        // Arrange
        let fixture = fixture.await;
        Mock::given(method("POST"))
            .and(path("/sign"))
            .and(body_partial_json(expected_body))
            .respond_with(
                ResponseTemplate::new(StatusCode::UNPROCESSABLE_ENTITY)
                    .set_body_string(REJECTION_BODY),
            )
            .expect(1)
            .mount(&fixture.mock_server)
            .await;

        let request = get_rejection_signature_request(Some(800000.into()), None, None);

        // Act
        let response = MaestroSignRequest::run(request, &fixture.state).await;

        // Assert
        let response = response.unwrap();
        let approval = response.payload.approval.as_result();
        let rejection = approval.unwrap_err();
        assert_eq!(rejection, REJECTION_BODY);
    }

    #[tokio::test]
    async fn handle_status_ok_success() {
        // Arrange
        let input = RPL_ENCODED_SIGNED_TRANSACTION;
        let response_body: Value = serde_json::from_str(input).unwrap();

        let status = StatusCode::OK;
        let text = response_body.to_string();

        // Act
        let response = parse_response(status, text).await;

        let response = response.unwrap();

        // Convert the hexadecimal string to bytes
        let bytes = hex::decode(TRANSACTION_HASH).expect("Failed to decode hex");

        // Create an H256 from the bytes
        let h256 = H256::from_slice(&bytes);

        let maestro_sing_response = MaestroSignResponse::Approved {
            signature: SIGNATURE.to_string(),
            transaction_hash: h256,
        };

        // Assert
        assert_eq!(response, maestro_sing_response);
    }

    #[tokio::test]
    async fn handle_status_ok_unprocessable_content_error() {
        test_unprocessable_content(StatusCode::OK).await;
    }

    #[tokio::test]
    async fn handle_status_unprocessable_enitity_unprocessable_content_error() {
        test_unprocessable_content(StatusCode::UNPROCESSABLE_ENTITY).await;
    }

    async fn test_unprocessable_content(status_code: StatusCode) {
        // Arrange
        let input = RPL_ENCODED_BAD_SIGNED_TRANSACTION;
        let response_body: Value = serde_json::from_str(input).unwrap();

        let text = response_body.to_string();

        // Act
        let response = parse_response(status_code, text).await;

        // Assert
        match response {
            Ok(_) => {
                panic!("This is not the right path for the test case");
            }
            Err(error) => {
                assert!(error
                    .to_string()
                    .contains("Maestro returned an unprocessable content result"));
            }
        };
    }

    #[tokio::test]
    async fn handle_unexpected_result_error() {
        // Arrange
        let input = RPL_ENCODED_BAD_SIGNED_TRANSACTION;
        let response_body: Value = serde_json::from_str(input).unwrap();

        let status = StatusCode::BAD_GATEWAY;
        let text = response_body.to_string();

        // Act
        let response = parse_response(status, text).await;

        // Assert
        match response {
            Ok(_) => {
                panic!("This is not the right path for the test case");
            }
            Err(error) => {
                assert!(error.to_string().contains("Unknown"));
            }
        };
    }
}
