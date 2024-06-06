use lambda_http::aws_lambda_events::apigw::ApiGatewayProxyRequestContext;
use lambda_http::request::RequestContext;
use lambda_http::{Body, Request, RequestExt};
use serde_json::Value;
use std::collections::HashMap;

pub fn build_request_custom_auth(auth_value: Value, body: Body) -> Request {
    let authorizer: HashMap<String, Value> = HashMap::from([("claims".to_string(), auth_value)]);
    let request_context = RequestContext::ApiGatewayV1(ApiGatewayProxyRequestContext {
        authorizer,
        ..ApiGatewayProxyRequestContext::default()
    });

    Request::new(body).with_request_context(request_context)
}
