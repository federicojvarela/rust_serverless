//! This file abstracts the logic of calling a step function in an asynchronous manner and
//! handling the errors that can occur.

use anyhow::anyhow;
use rusoto_stepfunctions::{StartExecutionInput, StepFunctions};
use serde_json::Value;

use crate::result::error::LambdaError;

pub struct StepFunctionConfig {
    pub step_function_arn: String,
}

pub async fn invoke_step_function_async(
    client_id: String,
    body: Value,
    step_functions_client: &impl StepFunctions,
    config: &StepFunctionConfig,
    order_id: String,
) -> Result<(), LambdaError> {
    let mut body = body;
    body["client_id"] = Value::String(client_id);
    body["order_id"] = Value::String(order_id);
    let body = serde_json::to_string(&body).map_err(|e| LambdaError::Unknown(anyhow!(e)))?;

    let _ = step_functions_client
        .start_execution(StartExecutionInput {
            input: Some(body),
            state_machine_arn: config.step_function_arn.clone(),
            ..StartExecutionInput::default()
        })
        .await
        .map_err(|e| LambdaError::Unknown(e.into()))?;

    Ok(())
}

pub async fn invoke_step_function_async_dyn_client(
    client_id: String,
    body: Value,
    step_functions_client: &(dyn StepFunctions + Sync + Send),
    config: &StepFunctionConfig,
    order_id: String,
) -> Result<(), LambdaError> {
    let mut body = body;
    body["client_id"] = Value::String(client_id);
    body["order_id"] = Value::String(order_id);
    let body = serde_json::to_string(&body).map_err(|e| LambdaError::Unknown(anyhow!(e)))?;

    let _ = step_functions_client
        .start_execution(StartExecutionInput {
            input: Some(body),
            state_machine_arn: config.step_function_arn.clone(),
            ..StartExecutionInput::default()
        })
        .await
        .map_err(|e| LambdaError::Unknown(e.into()))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::test_tools::http::constants::CLIENT_ID_FOR_MOCK_REQUESTS;
    use common::test_tools::mocks::step_client::MockStepsClient;
    use mockall::predicate;
    use rstest::*;
    use rusoto_core::{HttpDispatchError, RusotoError};
    use rusoto_stepfunctions::StartExecutionOutput;
    use serde_json::{json, Value};

    const STEP_FUNCTION_ARN: &str = "some::arn";
    const ORDER_ID: &str = "some_order_id";

    struct TestFixture {
        pub step_functions_client: MockStepsClient,
        pub config: StepFunctionConfig,
        pub request_body: Value,
        pub request_body_with_ids: Value,
    }

    #[fixture]
    async fn fixture() -> TestFixture {
        let step_functions_client = MockStepsClient::new();
        let config = StepFunctionConfig {
            step_function_arn: STEP_FUNCTION_ARN.to_owned(),
        };
        let request_body = json!({"input":"{}"});
        let request_body_with_ids = json!({ "client_id": CLIENT_ID_FOR_MOCK_REQUESTS, "order_id": ORDER_ID, "input": "{}" });
        TestFixture {
            step_functions_client,
            config,
            request_body,
            request_body_with_ids,
        }
    }

    #[rstest]
    #[tokio::test]
    async fn call_to_state_machine_succeeds(#[future] fixture: TestFixture) {
        let mut fixture = fixture.await;
        fixture
            .step_functions_client
            .expect_start_execution()
            .with(predicate::eq(StartExecutionInput {
                input: Some(fixture.request_body_with_ids.to_string()),
                state_machine_arn: STEP_FUNCTION_ARN.to_string(),
                ..StartExecutionInput::default()
            }))
            .times(1)
            .returning(move |_| Ok(StartExecutionOutput::default()));

        invoke_step_function_async(
            CLIENT_ID_FOR_MOCK_REQUESTS.to_owned(),
            fixture.request_body,
            &fixture.step_functions_client,
            &fixture.config,
            ORDER_ID.to_owned(),
        )
        .await
        .expect("Should succeed");
    }

    #[rstest]
    #[tokio::test]
    async fn call_to_state_machine_fails(#[future] fixture: TestFixture) {
        let mut fixture = fixture.await;
        fixture
            .step_functions_client
            .expect_start_execution()
            .with(predicate::eq(StartExecutionInput {
                input: Some(fixture.request_body_with_ids.to_string()),
                state_machine_arn: STEP_FUNCTION_ARN.to_string(),
                ..StartExecutionInput::default()
            }))
            .times(1)
            .returning(move |_| {
                Err(RusotoError::HttpDispatch(HttpDispatchError::new(
                    "timeout".to_owned(),
                )))
            });
        let error = invoke_step_function_async(
            CLIENT_ID_FOR_MOCK_REQUESTS.to_owned(),
            fixture.request_body,
            &fixture.step_functions_client,
            &fixture.config,
            ORDER_ID.to_owned(),
        )
        .await
        .unwrap_err();

        assert!(matches!(error, LambdaError::Unknown(_)));
        assert!(error.to_string().contains("timeout"));
    }
}
