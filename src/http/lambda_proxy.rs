use crate::http::errors::unknown_error_response;
use crate::result::error::LambdaError;
use anyhow::anyhow;
use lambda_http::http::StatusCode;
use lambda_http::Response;
use std::collections::HashMap;

pub struct LambdaProxyHttpResponse {
    pub status_code: StatusCode,
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
}

impl Default for LambdaProxyHttpResponse {
    fn default() -> Self {
        Self {
            status_code: StatusCode::OK,
            headers: HashMap::from([("Access-Control-Allow-Origin".to_owned(), "*".to_owned())]),
            body: None,
        }
    }
}

impl TryFrom<LambdaProxyHttpResponse> for Response<String> {
    type Error = Response<String>;

    fn try_from(proxy_response: LambdaProxyHttpResponse) -> Result<Self, Self::Error> {
        let mut response = Response::builder().status(proxy_response.status_code);

        for (k, v) in proxy_response.headers {
            response = response.header(k, v);
        }

        response
            .body(proxy_response.body.unwrap_or_default())
            .map_err(|e| {
                unknown_error_response(LambdaError::Unknown(anyhow!(
                    "Error building response: {e}"
                )))
            })
    }
}
