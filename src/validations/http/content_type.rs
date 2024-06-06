use crate::http::errors::unsupported_media_error_response;
use crate::lambda_structure::http_lambda_main::RequestExtractor;
use lambda_http::{Request, Response};

const CONTENT_TYPE_HEADER_VALUE: &str = "application/json";
const CONTENT_TYPE_HEADER_NAME: &str = "Content-Type";

pub fn validate_content_type(request: &Request) -> Result<(), Response<String>> {
    let content_type: String = request.extract_header(CONTENT_TYPE_HEADER_NAME)?;
    if !content_type
        .to_lowercase()
        .contains(CONTENT_TYPE_HEADER_VALUE)
    {
        Err(unsupported_media_error_response(None))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::validations::http::content_type::{validate_content_type, CONTENT_TYPE_HEADER_NAME};
    use http::{HeaderValue, StatusCode};
    use lambda_http::Request;
    use rstest::rstest;

    #[test]
    fn test_missing_content_type() {
        let request = Request::default();
        let error = validate_content_type(&request).unwrap_err();
        assert_eq!(error.status(), StatusCode::BAD_REQUEST);
        assert!(error
            .body()
            .contains("Content-Type not found in request headers"));
    }

    #[rstest]
    #[case::lowercase_header("application/json")]
    #[case::uppercase_header("APPLICATION/JSON")]
    #[case::mixed_case_header("Application/Json")]
    #[test]
    fn test_supported_content_type(#[case] header_value: &str) {
        let mut request = Request::default();
        request.headers_mut().insert(
            CONTENT_TYPE_HEADER_NAME,
            HeaderValue::from_str(header_value).unwrap(),
        );
        validate_content_type(&request).expect("Should be valid");
    }
}
