mod dtos;

use anyhow::anyhow;
use async_trait::async_trait;
use common::aws_clients::secrets_manager::get_secrets_provider;
use dtos::{KeyCreationRequest, KeyCreationResponse, MaestroKeyCreationResponse};
use ethers::utils::public_key_to_address;
use k256::ecdsa::VerifyingKey;
use mpc_signature_sm::{
    lambda_main,
    lambda_structure::{event::Event, lambda_trait::Lambda},
    maestro::{maestro_bootstrap, state::MaestroState},
    result::error::OrchestrationError,
};
use serde_json::json;

pub struct MaestroKeyCreationRequest;

#[async_trait]
impl Lambda for MaestroKeyCreationRequest {
    type PersistedMemory = MaestroState;
    type InputBody = Event<KeyCreationRequest>;
    type Output = Event<KeyCreationResponse>;
    type Error = OrchestrationError;

    async fn bootstrap() -> Result<Self::PersistedMemory, Self::Error> {
        let secrets_provider = get_secrets_provider().await;
        maestro_bootstrap(secrets_provider).await
    }

    async fn run(
        request: Self::InputBody,
        state: &Self::PersistedMemory,
    ) -> Result<Self::Output, Self::Error> {
        let response = state
            .http
            .post(format!("{}/generate", &state.config.maestro_url))
            .json(&json!({ "domain_name": request.payload.client_id }))
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(OrchestrationError::unknown(response.text().await?));
        }

        let response = response.json::<MaestroKeyCreationResponse>().await?;

        Ok(request
            .context
            .create_new_event_from_current(KeyCreationResponse {
                key_id: response.key_id,
                address: get_address_from_public_key(&response.public_key)?,
                public_key: response.public_key,
            }))
    }
}

pub fn get_address_from_public_key(public_key: &str) -> Result<String, OrchestrationError> {
    // Remove 0x prefix if present
    let public_key = if let Some(pk) = public_key.strip_prefix("0x") {
        pk
    } else {
        public_key
    };

    let bytes = hex::decode(public_key).map_err(|e| {
        OrchestrationError::from(anyhow!(e).context("Error converting public_key to bytes"))
    })?;
    let key = VerifyingKey::from_sec1_bytes(&bytes).map_err(|e| {
        OrchestrationError::from(
            anyhow!(e).context("Error converting public_key bytes to VerifyingKey"),
        )
    })?;

    Ok(format!("0x{:x}", public_key_to_address(&key)))
}

lambda_main!(MaestroKeyCreationRequest);

#[cfg(test)]
mod tests {
    use super::*;
    use http::StatusCode;
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
    use std::sync::Arc;
    use uuid::Uuid;
    use wiremock::{
        matchers::{method, path},
        Mock, MockServer, ResponseTemplate,
    };

    const EXPECTED_ADDRESS: &str = "0x90f8bf6a479f320ead074411a4b0e7944ea8c9c1";
    const PUB_KEY_COMPRESSED: &str =
        "03e68acfc0253a10620dff706b0a1b1f1f5833ea3beb3bde2250d5f271f3563606";
    const PUB_KEY_UNCOMPRESSED: &str = "04e68acfc0253a10620dff706b0a1b1f1f5833ea3beb3bde2250d5f271f3563606672ebc45e0b7ea2e816ecb70ca03137b1c9476eec63d4632e990020b7b6fba39";

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

    fn get_correct_request() -> KeyCreationRequest {
        KeyCreationRequest {
            client_id: "1ucce35ahvouias96lc293ouqv".to_owned(),
        }
    }

    #[rstest]
    #[tokio::test]
    async fn handle_key_creation_success(#[future] fixture: TestFixture) {
        // Arrange
        let fixture = fixture.await;
        let key_id = Uuid::new_v4();
        let public_key = format!("0x{}", PUB_KEY_UNCOMPRESSED);
        Mock::given(method("POST"))
            .and(path("/generate"))
            .respond_with(ResponseTemplate::new(StatusCode::OK).set_body_json(json!({
                "key_id": key_id,
                "public_key": public_key,
            })))
            .expect(1)
            .mount(&fixture.mock_server)
            .await;

        // Act
        let response = MaestroKeyCreationRequest::run(
            Event::test_event_from(get_correct_request()),
            &fixture.state,
        )
        .await;

        // Assert
        assert!(response.is_ok());
        let response = response.unwrap();
        assert_eq!(response.payload.key_id, key_id);
        assert_eq!(response.payload.public_key, public_key);
        assert_eq!(response.payload.address, EXPECTED_ADDRESS);
    }

    #[rstest]
    #[tokio::test]
    async fn handle_key_creation_200_unknown_body(#[future] fixture: TestFixture) {
        // Arrange
        let fixture = fixture.await;
        Mock::given(method("POST"))
            .and(path("/generate"))
            .respond_with(ResponseTemplate::new(StatusCode::OK).set_body_json(json!({
                "key_id": "dummy",
            })))
            .expect(1)
            .mount(&fixture.mock_server)
            .await;

        // Act
        let response = MaestroKeyCreationRequest::run(
            Event::test_event_from(get_correct_request()),
            &fixture.state,
        )
        .await;

        // Assert
        assert!(response.is_err());
        let orc_error = response.unwrap_err();
        assert!(matches!(orc_error, OrchestrationError::Unknown(_)));
        assert!(orc_error.to_string().contains("UUID parsing failed"));
    }

    #[rstest]
    #[case(500)]
    #[case(422)]
    #[case(400)]
    #[tokio::test]
    async fn handle_key_creation_unknown_if_fail(
        #[case] http_status: u16,
        #[future] fixture: TestFixture,
    ) {
        // Arrange
        let fixture = fixture.await;
        Mock::given(method("POST"))
            .and(path("/generate"))
            .respond_with(ResponseTemplate::new(http_status))
            .expect(1)
            .mount(&fixture.mock_server)
            .await;

        // Act
        let response = MaestroKeyCreationRequest::run(
            Event::test_event_from(get_correct_request()),
            &fixture.state,
        )
        .await;

        // Assert
        assert!(response.is_err());
        let response = response.unwrap_err();
        assert!(matches!(response, OrchestrationError::Unknown(_)));
    }

    #[rstest]
    #[case(PUB_KEY_COMPRESSED.to_string())]
    #[case(format!("0x{}", PUB_KEY_COMPRESSED))] // with prefix
    #[case(PUB_KEY_UNCOMPRESSED.to_string())]
    #[case(format!("0x{}", PUB_KEY_UNCOMPRESSED))] // with prefix
    #[test]
    fn test_address_from_public_key_success(#[case] public_key: String) {
        let address = get_address_from_public_key(&public_key).unwrap();
        assert_eq!(EXPECTED_ADDRESS, address);
    }

    #[test]
    fn test_address_from_public_key_invalid_len() {
        let response = get_address_from_public_key(""); // length = 0

        assert!(response.is_err());
        let orc_error = response.unwrap_err();
        assert!(matches!(orc_error, OrchestrationError::Unknown(_)));
        assert!(orc_error
            .to_string()
            .contains("Error converting public_key bytes to VerifyingKey"));
    }
}
