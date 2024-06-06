use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;

/// Information used by the `login` function to send the login request.
#[derive(Deserialize, Clone, Debug)]
pub struct MaestroLoginInformation {
    pub maestro_url: String,

    pub service_name: String,

    pub maestro_api_key: String,

    pub tenant_name: String,
}

#[derive(Deserialize)]
pub struct MaestroLoginResponse {
    pub access_token: String,
}

/// Logs in to Maestro. If login succeeds, token wil be saved in the state so it
/// is available in subsequent lambda calls.
///
/// Is passed in to the AuthenticationMiddleware, returns Result from that library.
pub async fn login(config: Arc<MaestroLoginInformation>) -> reqwest_middleware::Result<String> {
    let mut params = HashMap::new();
    params.insert("username", config.service_name.clone());
    params.insert("password", config.maestro_api_key.clone());
    params.insert("grant_type", "password".to_string());

    let response = reqwest::Client::new()
        .post(format!(
            "{}/{}/login",
            &config.maestro_url, &config.tenant_name
        ))
        .form(&params)
        .send()
        .await?
        .json::<MaestroLoginResponse>()
        .await?;

    Ok(response.access_token)
}

#[cfg(test)]
mod tests {
    use crate::maestro::session::{login, MaestroLoginInformation};
    use reqwest::StatusCode;
    use rstest::*;
    use serde_json::{json, Value};
    use std::sync::Arc;
    use wiremock::matchers::body_string_contains;
    use wiremock::{
        matchers::{method, path},
        Mock, MockServer, ResponseTemplate,
    };

    struct TestFixture {
        pub login_information: MaestroLoginInformation,
        pub mock_server: MockServer,
    }

    #[fixture]
    async fn fixture() -> TestFixture {
        let mock_server = MockServer::start().await;
        let login_information = MaestroLoginInformation {
            maestro_url: mock_server.uri(),
            service_name: "mpc-wallet".to_owned(),
            maestro_api_key: "some.api.key".to_owned(),
            tenant_name: "tenant".to_owned(),
        };

        TestFixture {
            login_information,
            mock_server,
        }
    }

    #[rstest]
    #[tokio::test]
    async fn successful_login(#[future] fixture: TestFixture) {
        let fixture = fixture.await;
        let token = "some.valid.token";
        mock_maestro_response(StatusCode::OK, json!({ "access_token": token }), &fixture).await;

        let result = login(Arc::new(fixture.login_information)).await.unwrap();
        assert_eq!(token, result);
    }

    #[rstest]
    #[tokio::test]
    async fn fail_to_login(#[future] fixture: TestFixture) {
        let fixture = fixture.await;
        mock_maestro_response(
            StatusCode::BAD_REQUEST,
            json!({ "error": "invalid request" }),
            &fixture,
        )
        .await;

        let result = login(Arc::new(fixture.login_information)).await;
        assert!(result.is_err())
    }

    async fn mock_maestro_response(status_code: StatusCode, json: Value, fixture: &TestFixture) {
        Mock::given(method("POST"))
            .and(path(format!(
                "/{}/login",
                fixture.login_information.tenant_name
            )))
            .and(body_string_contains(format!(
                "username={}",
                fixture.login_information.service_name
            )))
            .and(body_string_contains(format!(
                "password={}",
                fixture.login_information.maestro_api_key
            )))
            .and(body_string_contains("grant_type=password"))
            .respond_with(ResponseTemplate::new(status_code).set_body_json(json))
            .expect(1)
            .mount(&fixture.mock_server)
            .await;
    }
}
