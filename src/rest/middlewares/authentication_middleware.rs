use anyhow::anyhow;
use async_trait::async_trait;
use http::StatusCode;
use lambda_http::http::HeaderValue;
use reqwest::{Request, Response};
use reqwest_middleware::{Error, Middleware, Next, Result};
use std::future::Future;
use task_local_extensions::Extensions;
use tokio::sync::RwLock;

/// This middleware hides the logic for refreshing/obtaining authentication tokens.
///
/// Clients using this will have the authentication token:
/// - Automatically injected in every request they make.
/// - Automatically refreshed when getting a 401 Unauthorized response.
pub struct AuthenticationMiddleware<F, I>
where
    F: Future<Output = Result<String>> + Send + 'static,
    I: Sync + Send + Clone + 'static,
{
    /// Callback used for refreshing the token
    refresh_token_function: &'static (dyn Fn(I) -> F + Sync + Send),

    /// Information needed by `refresh_token_function` function to obtain and save the new token
    refresh_token_information: I,

    /// Token used for authentication.
    token: RwLock<Option<String>>,
}

impl<F, I> AuthenticationMiddleware<F, I>
where
    F: Future<Output = Result<String>> + Send + 'static,
    I: Sync + Send + Clone + 'static,
{
    pub fn new(
        refresh_token_function: &'static (impl Fn(I) -> F + Sync + Send),
        refresh_token_information: I,
        token: Option<String>,
    ) -> Self {
        Self {
            refresh_token_function,
            refresh_token_information,
            token: RwLock::new(token),
        }
    }

    async fn inject_token(&self, req: &Request) -> Result<Request> {
        let mut request = req.try_clone().ok_or_else(|| {
            Error::Middleware(anyhow!(
                "Request object is not clonable. Are you passing a streaming body?"
            ))
        })?;

        if let Some(ref token) = *self.token.read().await {
            let bearer = HeaderValue::from_str(&format!("Bearer {token}")).map_err(|e| {
                Error::Middleware(anyhow!("Unable to create Authorization header. {}", e))
            })?;
            request.headers_mut().append("Authorization", bearer);
        }

        Ok(request)
    }
}

#[async_trait]
impl<F, I> Middleware for AuthenticationMiddleware<F, I>
where
    F: Future<Output = Result<String>> + Send + 'static,
    I: Sync + Send + Clone + 'static,
{
    async fn handle(
        &self,
        req: Request,
        extensions: &mut Extensions,
        next: Next<'_>,
    ) -> Result<Response> {
        // Inject the token and make the request
        let request = self.inject_token(&req).await?;
        let mut response = next.clone().run(request, extensions).await?;

        // If we fail with unauthorized code, we execute the login function to retrieve a new token
        // and retry the request.
        // If the request fails again we return it
        if response.status() == StatusCode::UNAUTHORIZED {
            let refresh_token = self.refresh_token_function;
            let new_token = refresh_token(self.refresh_token_information.clone()).await?;

            let mut token = self.token.write().await;
            *token = Some(new_token);
            drop(token);

            let request = self.inject_token(&req).await?;
            response = next.run(request, extensions).await?;
        }

        Ok(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::StatusCode;
    use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
    use wiremock::{
        matchers::{self, method, path},
        Mock, MockServer, ResponseTemplate,
    };

    const JWT: &str = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c";

    async fn refresh_token(_config: &()) -> reqwest_middleware::Result<String> {
        Ok(JWT.to_string())
    }

    async fn setup_mock_server(mock_server: &MockServer) {
        let auth_header = HeaderValue::from_str(&format!("Bearer {JWT}")).unwrap();

        Mock::given(method("GET"))
            .and(path("/sign"))
            .and(matchers::header("authorization", auth_header))
            .respond_with(ResponseTemplate::new(StatusCode::OK))
            .expect(1)
            .mount(mock_server)
            .await;

        Mock::given(method("GET"))
            .and(path("/sign"))
            .respond_with(ResponseTemplate::new(StatusCode::UNAUTHORIZED))
            .expect(1)
            .mount(mock_server)
            .await;
    }

    fn build_rest_client(token: Option<String>) -> ClientWithMiddleware {
        ClientBuilder::new(reqwest::Client::new())
            .with(AuthenticationMiddleware::new(&refresh_token, &(), token))
            .build()
    }

    #[tokio::test]
    async fn token_is_created_if_not_present() {
        // Arrange
        let mock_server = MockServer::start().await;
        setup_mock_server(&mock_server).await;

        let client = build_rest_client(None);

        // Act
        let res = client
            .get(format!("{}/sign", mock_server.uri()))
            .send()
            .await
            .unwrap();

        // Assert
        assert_eq!(StatusCode::OK, res.status());
    }

    #[tokio::test]
    async fn token_is_refreshed() {
        // Arrange
        let mock_server = MockServer::start().await;
        setup_mock_server(&mock_server).await;

        let invalid_token = "INVALID_TOKEN".to_owned();
        let client = build_rest_client(Some(invalid_token.clone()));

        // Act
        let res = client
            .get(format!("{}/sign", mock_server.uri()))
            .send()
            .await
            .unwrap();

        // Assert
        assert_eq!(StatusCode::OK, res.status());
    }
}
