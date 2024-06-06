use crate::{config::SupportedChain, lambda_structure::http_lambda_main::RequestExtractor};
use lambda_http::{Request, Response};
use validator::ValidationError;

use crate::http::errors::validation_error_response;

const CHAIN_ID_PATH_PARAM: &str = "chain_id";

pub fn validate_chain_id_is_supported(request: &Request) -> Result<(), Response<String>> {
    let chain_id: u64 = request.extract_path_param(CHAIN_ID_PATH_PARAM)?;

    if !chain_id.is_supported() {
        return Err(validation_error_response(
            format!("chain_id {chain_id} is not supported",),
            None,
        ));
    }

    Ok(())
}

/// This is used for custom validations with validator crate
pub fn is_supported_chain_id(chain_id: u64) -> Result<(), ValidationError> {
    if !chain_id.is_supported() {
        return Err(ValidationError::new("chain_id is not supported"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::validations::http::supported_chain_id::{
        validate_chain_id_is_supported, CHAIN_ID_PATH_PARAM,
    };
    use common::test_tools::http::constants::CHAIN_ID_FOR_MOCK_REQUESTS;
    use http::{Request, StatusCode};
    use lambda_http::{Body, RequestExt};
    use std::collections::HashMap;

    #[test]
    fn test_missing_path_param() {
        let request = Request::default();
        let error = validate_chain_id_is_supported(&request).unwrap_err();
        assert_eq!(error.status(), StatusCode::BAD_REQUEST);
        assert!(error.body().contains("chain_id not found in request path"));
    }

    #[test]
    fn test_not_supported_chain_id() {
        let chain_id: u64 = 28731237918;
        let request =
            Request::new(Body::Text("".to_owned())).with_path_parameters::<HashMap<_, _>>(
                HashMap::from([(CHAIN_ID_PATH_PARAM.to_owned(), chain_id.to_string())]),
            );
        let error = validate_chain_id_is_supported(&request).unwrap_err();
        assert_eq!(error.status(), StatusCode::BAD_REQUEST);
        let message = format!("chain_id {chain_id} is not supported");
        assert!(error.body().contains(&message));
    }

    #[test]
    fn test_supported_chain_id() {
        let chain_id: u64 = CHAIN_ID_FOR_MOCK_REQUESTS;
        let request =
            Request::new(Body::Text("".to_owned())).with_path_parameters::<HashMap<_, _>>(
                HashMap::from([(CHAIN_ID_PATH_PARAM.to_owned(), chain_id.to_string())]),
            );
        validate_chain_id_is_supported(&request).expect("Chain_id should be supported");
    }
}
