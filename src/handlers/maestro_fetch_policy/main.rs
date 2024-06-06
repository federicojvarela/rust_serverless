mod dtos;

use crate::dtos::{
    MaestroFetchPolicyError, MaestroFetchPolicyRequest, MaestroGetPolicyResponse,
    MaestroPolicyInfo, ProcessApproverResponse,
};
use anyhow::anyhow;
use async_trait::async_trait;
use common::aws_clients::secrets_manager::get_secrets_provider;
use http::StatusCode;
use model::order::policy::{Approval, Policy};
use mpc_signature_sm::maestro::dtos::MaestroAuthorizingEntityLevel;
use mpc_signature_sm::result::error::UnknownOrchestrationError;
use mpc_signature_sm::{
    lambda_main,
    lambda_structure::lambda_trait::Lambda,
    maestro::{maestro_bootstrap, state::MaestroState},
    result::error::OrchestrationError,
};
use openssl::base64;

pub struct MaestroFetchPolicy;

#[async_trait]
impl Lambda for MaestroFetchPolicy {
    type PersistedMemory = MaestroState;
    type InputBody = MaestroFetchPolicyRequest;
    type Output = ProcessApproverResponse;
    type Error = OrchestrationError;

    async fn bootstrap() -> Result<Self::PersistedMemory, Self::Error> {
        let secrets_provider = get_secrets_provider().await;
        maestro_bootstrap(secrets_provider).await
    }

    async fn run(
        request: Self::InputBody,
        state: &Self::PersistedMemory,
    ) -> Result<Self::Output, Self::Error> {
        let policy_name = request.policy_name.clone();
        let domain_name = request.domain_name.clone();

        let maestro_url = format!(
            "{}/{}/policy/{}",
            &state.config.maestro_url, domain_name, policy_name
        );

        let response = state.http.get(maestro_url.clone()).send().await?;

        let status_code = response.status();
        if status_code != StatusCode::OK {
            let response_text = response.text().await?;
            return Err(OrchestrationError::Unknown(
                UnknownOrchestrationError::GenericError(anyhow!(response_text)),
            ));
        }

        let maestro_get_policy = response.json::<MaestroGetPolicyResponse>().await?;

        let fetch_policy_info = decode_policy_info(maestro_get_policy.serialized_policy)?;

        let all_domain_approvers = fetch_policy_info.domain_approvals.required;
        let domain_approvers: Vec<Approval> = all_domain_approvers
            .iter()
            .map(|approver_name| Approval {
                name: String::from(approver_name),
                level: MaestroAuthorizingEntityLevel::Domain.to_string(),
                response: None,
            })
            .collect();

        let all_tenant_approvers = fetch_policy_info.tenant_approvals.required;
        let tenant_approvers: Vec<Approval> = all_tenant_approvers
            .iter()
            .map(|approver_name| Approval {
                name: String::from(approver_name),
                level: MaestroAuthorizingEntityLevel::Tenant.to_string(),
                response: None,
            })
            .collect();

        let order_policy_info = Policy {
            name: policy_name,
            approvals: [tenant_approvers, domain_approvers].concat(),
        };
        let maestro_fetch_policy_response = ProcessApproverResponse {
            policy: order_policy_info,
        };

        Ok(maestro_fetch_policy_response)
    }
}

fn decode_policy_info(text: String) -> Result<MaestroPolicyInfo, MaestroFetchPolicyError> {
    let decoded = base64::decode_block(text.as_str())
        .map_err(|_| MaestroFetchPolicyError::InvalidBase64DecodeText)?;

    let decode_str =
        String::from_utf8(decoded).map_err(|_| MaestroFetchPolicyError::InvalidJson)?;

    let policy_info = serde_json::from_str(decode_str.as_str())
        .map_err(|_| MaestroFetchPolicyError::DecodeJsonError)?;

    Ok(policy_info)
}

lambda_main!(MaestroFetchPolicy);

#[cfg(test)]
mod tests {
    use crate::dtos::{MaestroFetchPolicyError, MaestroFetchPolicyRequest};
    use crate::{decode_policy_info, MaestroFetchPolicy};
    use http::StatusCode;
    use mpc_signature_sm::lambda_structure::lambda_trait::Lambda;
    use mpc_signature_sm::maestro::config::MaestroConfig;
    use mpc_signature_sm::maestro::session::{login, MaestroLoginInformation};
    use mpc_signature_sm::maestro::state::MaestroState;
    use mpc_signature_sm::rest::middlewares::AuthenticationMiddleware;
    use rstest::{fixture, rstest};
    use serde_json::json;
    use std::sync::Arc;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    const TEST_SERIALIZED_POLICY: &str = "eyJkb21haW5fYXBwcm92YWxzIjp7Im9wdGlvbmFsIjpbXSwicmVxdWlyZWQiOltdfSwibWluX29wdGlvbmFsX2FwcHJvdmFscyI6MCwicG9saWN5X25hbWUiOiJEZWZhdWx0VGVuYW50U29sb0FwcHJvdmFsIiwidGVuYW50X2FwcHJvdmFscyI6eyJvcHRpb25hbCI6W10sInJlcXVpcmVkIjpbImZvcnRlX3dhYXNfdHhuX2FwcHJvdmVyIl19fQ==";
    const INVALID_JSON_SERIALIZED_POLICY: &str = "eyJkb21haW5fYXBwcm92YWxzIjpbXSwibWluX29wdGlvbmFsX2FwcHJvdmFscyI6MCwicG9saWN5X25hbWUiOiJEZWZhdWx0VGVuYW50U29sb0FwcHJvdmFsIiwidGVuYW50X2FwcHJvdmFscyI6eyJvcHRpb25hbCI6W10sInJlcXVpcmVkIjpbImZvcnRlX3dhYXNfdHhuX2FwcHJvdmVyIl19fQ==";

    const INVALID_BASE64_DECODE: &str = "_";

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
            maestro_api_key_secret_name: "dummy_key".to_owned(),
            maestro_tenant_name: "tenant".to_owned(),
        };

        let http_client = reqwest_middleware::ClientBuilder::new(reqwest::Client::new())
            .with(AuthenticationMiddleware::new(
                &login,
                Arc::new(MaestroLoginInformation {
                    maestro_url: config.maestro_url.clone(),
                    service_name: config.service_name.clone(),
                    maestro_api_key: "dummy_secret".to_owned(),
                    tenant_name: "forte".to_owned(),
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

    #[rstest]
    fn valid_decode_policy_info() {
        let valid_policy_info = decode_policy_info(TEST_SERIALIZED_POLICY.to_string());

        assert!(valid_policy_info.is_ok());
    }

    #[rstest]
    fn invalid_json_decode_policy_info() {
        let invalid_json_policy_info =
            decode_policy_info(INVALID_JSON_SERIALIZED_POLICY.to_string());

        assert!(matches!(
            invalid_json_policy_info,
            Err(MaestroFetchPolicyError::DecodeJsonError)
        ));
        assert!(invalid_json_policy_info.is_err());
    }

    #[rstest]
    fn invalid_decode_policy_info() {
        let invalid_decode_policy_info = decode_policy_info(INVALID_BASE64_DECODE.to_string());

        assert!(matches!(
            invalid_decode_policy_info,
            Err(MaestroFetchPolicyError::InvalidBase64DecodeText)
        ));
        assert!(invalid_decode_policy_info.is_err());
    }

    #[rstest]
    #[tokio::test]
    async fn fetch_policy_200_valid(#[future] fixture: TestFixture) {
        let fixture = fixture.await;
        let policy_name = String::from("dummy_policy");
        let domain_name = String::from("123124312413");

        Mock::given(method("GET"))
            .and(path(format!("{}/policy/{}", domain_name, policy_name)))
            .respond_with(ResponseTemplate::new(StatusCode::OK).set_body_json(json!({
                "serialized_policy": TEST_SERIALIZED_POLICY,
                "policy_name": "Dummy_Policy",
                "display_name": "My dummy policy"
            })))
            .expect(1)
            .mount(&fixture.mock_server)
            .await;

        let request = MaestroFetchPolicyRequest {
            policy_name,
            domain_name,
        };

        let response = MaestroFetchPolicy::run(request, &fixture.state).await;

        let response = response.unwrap();
        let approvals = response.policy.approvals;

        assert_eq!(1, approvals.len());
    }

    #[rstest]
    #[tokio::test]
    async fn fetch_policy_200_invalid_policy_decode(#[future] fixture: TestFixture) {
        let fixture = fixture.await;
        let policy_name = String::from("dummy_policy");
        let domain_name = String::from("123124312413");

        Mock::given(method("GET"))
            .and(path(format!("{}/policy/{}", domain_name, policy_name)))
            .respond_with(ResponseTemplate::new(StatusCode::OK).set_body_json(json!({
                "serialized_policy": INVALID_BASE64_DECODE,
                "policy_name": "Dummy_Policy",
                "display_name": "My dummy policy"
            })))
            .expect(1)
            .mount(&fixture.mock_server)
            .await;

        let request = MaestroFetchPolicyRequest {
            policy_name,
            domain_name,
        };

        let response = MaestroFetchPolicy::run(request, &fixture.state).await;

        assert!(response.is_err());
    }

    #[rstest]
    #[tokio::test]
    async fn fetch_policy_500(#[future] fixture: TestFixture) {
        let fixture = fixture.await;
        let policy_name = String::from("dummy_policy");
        let domain_name = String::from("123124312413");

        Mock::given(method("GET"))
            .and(path(format!("{}/policy/{}", domain_name, policy_name)))
            .respond_with(
                ResponseTemplate::new(StatusCode::INTERNAL_SERVER_ERROR).set_body_json(json!({
                    "serialized_policy": TEST_SERIALIZED_POLICY,
                    "policy_name": "Dummy_Policy",
                    "display_name": "My dummy policy"
                })),
            )
            .expect(1)
            .mount(&fixture.mock_server)
            .await;

        let request = MaestroFetchPolicyRequest {
            policy_name,
            domain_name,
        };

        let response = MaestroFetchPolicy::run(request, &fixture.state).await;

        assert!(response.is_err());
    }
}
