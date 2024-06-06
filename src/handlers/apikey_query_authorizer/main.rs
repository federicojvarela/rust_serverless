// TODO: Use lambda trait
// Based on https://github.com/awslabs/aws-apigateway-lambda-authorizer-blueprints/blob/master/blueprints/rust/main.rs

use aws_lambda_events::apigw::{
    ApiGatewayCustomAuthorizerPolicy, ApiGatewayCustomAuthorizerRequestTypeRequest,
    ApiGatewayCustomAuthorizerResponse, IamPolicyStatement,
};
use common::aws_clients::secrets_manager::get_secrets_provider;
use lambda_runtime::{run, service_fn, Error, LambdaEvent};
use mpc_signature_sm::feature_flags::FeatureFlags;
use serde_json::json;
use tracing_subscriber::{filter::LevelFilter, fmt, prelude::*, reload};

#[tokio::main]
async fn main() -> Result<(), Error> {
    let (filter, reload_handle) = reload::Layer::new(LevelFilter::WARN);
    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::Layer::default().without_time())
        .init();
    let reload_handle = &reload_handle;

    let service = move |event: LambdaEvent<ApiGatewayCustomAuthorizerRequestTypeRequest>| async move {
        let secrets_provider = get_secrets_provider().await;
        let feature_flags = FeatureFlags::new(secrets_provider).await;
        if feature_flags.get_verbose_mode_flag() {
            reload_handle
                .modify(|filter| *filter = LevelFilter::INFO)
                .unwrap_or_else(|e| tracing::error!(error = ?e, "{:?}", e));
        }
        custom_authorizer(event).await
    };

    run(service_fn(service)).await
}

async fn custom_authorizer(
    event: LambdaEvent<ApiGatewayCustomAuthorizerRequestTypeRequest>,
) -> Result<ApiGatewayCustomAuthorizerResponse, Error> {
    let event = event.payload;
    tracing::info!(event = ?event, method_arn = event.method_arn.as_ref().unwrap(),  "Event: {:?}", event);

    let tmp: Vec<&str> = event.method_arn.as_ref().unwrap().split(':').collect();
    let api_gateway_arn_tmp: Vec<&str> = tmp[5].split('/').collect();
    let aws_account_id = tmp[4];
    let region = tmp[3];
    let rest_api_id = api_gateway_arn_tmp[0];
    let stage = api_gateway_arn_tmp[1];
    let method = api_gateway_arn_tmp[2];
    let resource = event.path.unwrap();

    let policy = ApiGatewayPolicyBuilder::new(region, aws_account_id, rest_api_id, stage)
        .allow_method(method, resource)
        .build();
    tracing::info!(policy = ?policy, "Policy {:?}", policy);

    let api_key = event.query_string_parameters.first("apiKey").unwrap();

    let response = ApiGatewayCustomAuthorizerResponse {
        principal_id: None,
        policy_document: policy,
        context: json!({
            "stringKey": "stringval",
            "numberKey": 123,
            "booleanKey": true
        }),
        usage_identifier_key: Some(api_key.to_string()),
    };
    tracing::info!(response = ?response, "Response {:?}", response);

    Ok(response)
}

struct ApiGatewayPolicyBuilder {
    region: String,
    aws_account_id: String,
    rest_api_id: String,
    stage: String,
    policy: ApiGatewayCustomAuthorizerPolicy,
}

impl ApiGatewayPolicyBuilder {
    pub fn new(
        region: &str,
        account_id: &str,
        api_id: &str,
        stage: &str,
    ) -> ApiGatewayPolicyBuilder {
        Self {
            region: region.to_string(),
            aws_account_id: account_id.to_string(),
            rest_api_id: api_id.to_string(),
            stage: stage.to_string(),
            policy: ApiGatewayCustomAuthorizerPolicy {
                version: Some("2012-10-17".to_string()),
                statement: vec![],
            },
        }
    }

    pub fn add_method<T: Into<String>>(mut self, effect: &str, method: &str, resource: T) -> Self {
        let resource_arn = format!(
            "arn:aws:execute-api:{}:{}:{}/{}/{}/{}",
            &self.region,
            &self.aws_account_id,
            &self.rest_api_id,
            &self.stage,
            method,
            resource.into().trim_start_matches('/')
        );

        let stmt = IamPolicyStatement {
            effect: Some(effect.to_owned()),
            action: vec!["execute-api:Invoke".to_string()],
            resource: vec![resource_arn],
        };

        self.policy.statement.push(stmt);
        self
    }

    // pub fn allow_all_methods(self) -> Self {
    //     self.add_method("Allow", "*", "*")
    // }

    // pub fn deny_all_methods(self) -> Self {
    //     self.add_method("Deny", "*", "*")
    // }

    pub fn allow_method(self, method: &str, resource: String) -> Self {
        self.add_method("Allow", method, resource)
    }

    // pub fn deny_method(self, method: &str, resource: String) -> Self {
    //     self.add_method("Deny", method, resource)
    // }

    // Creates and executes a new child thread.
    pub fn build(self) -> ApiGatewayCustomAuthorizerPolicy {
        self.policy
    }
}
