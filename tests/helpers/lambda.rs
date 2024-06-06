use serde::{de::DeserializeOwned, Serialize};

pub struct LambdaClient {
    lambda_watch_url: String,
    rest_client: reqwest::Client,
}

#[derive(Debug)]
pub enum LambdaError {
    RestError(reqwest::Error),
}

#[derive(Debug)]
pub struct LambdaResponse<R> {
    pub body: R,
    pub status: u16,
}

impl LambdaClient {
    pub fn new(lambda_watch_url: String) -> Self {
        Self {
            rest_client: reqwest::Client::new(),
            lambda_watch_url,
        }
    }

    pub async fn invoke<T: Serialize, R: DeserializeOwned>(
        &self,
        fn_name: &str,
        body: T,
    ) -> Result<LambdaResponse<R>, LambdaError> {
        let url = format!(
            "{}/2015-03-31/functions/{fn_name}/invocations",
            &self.lambda_watch_url
        );

        let response = self
            .rest_client
            .post(url)
            .json(&body)
            .send()
            .await
            .map_err(LambdaError::RestError)?;

        Ok(LambdaResponse {
            status: response.status().into(),
            body: response.json::<R>().await.map_err(LambdaError::RestError)?,
        })
    }
}
