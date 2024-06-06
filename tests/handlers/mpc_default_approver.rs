use ana_tools::config_loader::ConfigLoader;
use http::StatusCode;
use rstest::fixture;
use rstest::rstest;
use rusoto_secretsmanager::CreateSecretRequest;
use rusoto_secretsmanager::DeleteSecretRequest;
use rusoto_secretsmanager::SecretsManager;
use rusoto_sqs::{CreateQueueRequest, DeleteQueueRequest, Sqs};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use model::order::helpers::sponsored_typed_data;
use mpc_signature_sm::result::error::ErrorFromHttpHandler;

use crate::fixtures::lambda::{fixture, LambdaFixture};
use crate::fixtures::secrets::{secrets_manager_fixture, SecretsManagerFixture};
use crate::fixtures::sqs::{sqs_fixture, SqsFixture};
use crate::handlers::common_assertions::assert_error_from_http_handler;
use crate::helpers::lambda::LambdaResponse;

const FUNCTION_NAME: &str = "mpc_default_approver";
const ADDRESS_TO: &str = "0x1c965d1241D0040A3fC2A030BaeeEfB35C155a4e";
const ADDRESS_FROM: &str = "0xFF6A5DB899FB29F67A224CDA089572C2BC5A7A5E";
const INVALID_FORMAT_OR_MISSING_FIELD_MESSAGE: &str = "data did not match any variant";

const APPROVER_QUEUE_REQUEST_NAME: &str = "compliance-request";
const APPROVER_QUEUE_RESPONSE_NAME: &str = "compliance-response";

pub struct LocalFixture;

#[derive(Deserialize)]
pub struct Config {
    pub approver_private_key_secret_name: String,
}

#[fixture]
async fn local_fixture(
    secrets_manager_fixture: &SecretsManagerFixture,
    sqs_fixture: &SqsFixture,
) -> LocalFixture {
    let config = ConfigLoader::load_test::<Config>();
    let private_key = std::fs::read_to_string("./tests/data/approver_key.pem")
        .expect("Unable to read approver key");

    // Re-create the secret
    secrets_manager_fixture
        .secrets_manager
        .delete_secret(DeleteSecretRequest {
            secret_id: config.approver_private_key_secret_name.clone(),
            force_delete_without_recovery: Some(true),
            ..DeleteSecretRequest::default()
        })
        .await
        .unwrap();

    secrets_manager_fixture
        .secrets_manager
        .create_secret(CreateSecretRequest {
            name: config.approver_private_key_secret_name,
            secret_string: Some(private_key),
            client_request_token: Some(Uuid::new_v4().to_string()),
            ..CreateSecretRequest::default()
        })
        .await
        .unwrap();

    // Re-create the queue
    let queues = sqs_fixture
        .sqs_client
        .list_queues(rusoto_sqs::ListQueuesRequest::default())
        .await
        .expect("Unable to list SQS queues");

    if queues.queue_urls.is_some() {
        for queue_url in queues.queue_urls.unwrap() {
            sqs_fixture
                .sqs_client
                .delete_queue(DeleteQueueRequest { queue_url })
                .await
                .expect("Unable to delete SQS queue");
        }
    }

    sqs_fixture
        .sqs_client
        .create_queue(CreateQueueRequest {
            queue_name: APPROVER_QUEUE_REQUEST_NAME.to_owned(),
            ..Default::default()
        })
        .await
        .expect("Unable to create SQS queue");

    sqs_fixture
        .sqs_client
        .create_queue(CreateQueueRequest {
            queue_name: APPROVER_QUEUE_RESPONSE_NAME.to_owned(),
            ..Default::default()
        })
        .await
        .expect("Unable to create SQS queue");

    LocalFixture
}

fn get_legacy_request(to: &str, from: &str) -> String {
    json!({
        "transaction": {
            "to": to,
            "gas": "300000",
            "gas_price": "300000000",
            "value": "111111",
            "nonce": "15",
            "data": "0x6406516041610651325106165165106516169610",
            "chain_id": 1
        },
        "from": from,
        "contextual_data":{
            "order_id":"59dc17dd-a960-4afa-84ab-01f5be290e8b"
        }
    })
    .to_string()
}

fn get_sponsored_request(from: &str) -> String {
    json!( {
        "transaction": {
            "chain_id":1,
            "typed_data": sponsored_typed_data(),
            "to": ADDRESS_TO,
        },
        "from": from,
        "contextual_data":{
            "order_id":"59dc17dd-a960-4afa-84ab-01f5be290e8b"
        }
    }
    )
    .to_string()
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn mpc_default_approver_legacy_ok(
    fixture: &LambdaFixture,
    #[future] local_fixture: LocalFixture,
) {
    let _local_fixture = local_fixture.await;
    let request = get_legacy_request(ADDRESS_TO, ADDRESS_FROM);
    let response: LambdaResponse<Value> = fixture
        .lambda
        .invoke(FUNCTION_NAME, json!([{ "body": request }]))
        .await
        .unwrap_or_else(|_| panic!("There was an error invoking {FUNCTION_NAME}"));

    // TODO: This should be replaced with an sqs client that probably comes in a fixture, since
    // this is a POC this is just testing the integration using the REST API
    // TODO: When doing the todo from above, delete the wiremock mock located at
    // tests/wiremock/localstack/get_queue
    use reqwest::header::HeaderMap;
    use reqwest::header::HeaderValue;

    let mut headers = HeaderMap::new();
    headers.insert("Accept", HeaderValue::from_static("application/json"));
    let http_client = reqwest::ClientBuilder::new()
        .default_headers(headers)
        .build()
        .expect("Unable to build http client");

    let result = http_client
        .get(format!(
            "{}/queue/us-west-2/000000000000/compliance-response?Action=ReceiveMessage",
            fixture.localstack_url
        ))
        .send()
        .await
        .expect("Unable to make request to SQS")
        .json::<Value>()
        .await
        .expect("Could not serialize response");

    assert!(!result["ReceiveMessageResponse"]["ReceiveMessageResult"].is_null());

    assert_eq!(StatusCode::OK, response.status);
}

#[rstest]
#[tokio::test(flavor = "multi_thread")]
pub async fn mpc_default_approver_sponsored_ok(
    fixture: &LambdaFixture,
    #[future] local_fixture: LocalFixture,
) {
    let _local_fixture = local_fixture.await;
    let request = get_sponsored_request(ADDRESS_FROM);
    let response: LambdaResponse<Value> = fixture
        .lambda
        .invoke(FUNCTION_NAME, json!([{ "body": request }]))
        .await
        .unwrap_or_else(|_| panic!("There was an error invoking {FUNCTION_NAME}"));

    // TODO: This should be replaced with an sqs client that probably comes in a fixture, since
    // this is a POC this is just testing the integration using the REST API
    // TODO: When doing the todo from above, delete the wiremock mock located at
    // tests/wiremock/localstack/get_queue
    use reqwest::header::HeaderMap;
    use reqwest::header::HeaderValue;

    let mut headers = HeaderMap::new();
    headers.insert("Accept", HeaderValue::from_static("application/json"));
    let http_client = reqwest::ClientBuilder::new()
        .default_headers(headers)
        .build()
        .expect("Unable to build http client");

    let result = http_client
        .get(format!(
            "{}/queue/us-west-2/000000000000/compliance-response?Action=ReceiveMessage",
            fixture.localstack_url
        ))
        .send()
        .await
        .expect("Unable to make request to SQS")
        .json::<Value>()
        .await
        .expect("Could not serialize response");

    assert!(!result["ReceiveMessageResponse"]["ReceiveMessageResult"].is_null());

    assert_eq!(StatusCode::OK, response.status);
}

#[rstest]
#[case::invalid_to("INVALID", ADDRESS_FROM)]
#[tokio::test(flavor = "multi_thread")]
pub async fn mpc_default_approver_invalid_to_address(
    fixture: &LambdaFixture,
    #[future] local_fixture: LocalFixture,
    #[case] to: &str,
    #[case] from: &str,
) {
    let _local_fixture = local_fixture.await;
    let request = get_legacy_request(to, from);
    let response: LambdaResponse<ErrorFromHttpHandler> = fixture
        .lambda
        .invoke(FUNCTION_NAME, json!([{ "body": request }]))
        .await
        .unwrap_or_else(|_| panic!("There was an error invoking {FUNCTION_NAME}"));

    assert_error_from_http_handler(response, INVALID_FORMAT_OR_MISSING_FIELD_MESSAGE);
}

#[rstest]
#[case::invalid_from(ADDRESS_TO, "INVALID")]
#[tokio::test(flavor = "multi_thread")]
pub async fn mpc_default_approver_invalid_from_address(
    fixture: &LambdaFixture,
    #[future] local_fixture: LocalFixture,
    #[case] to: &str,
    #[case] from: &str,
) {
    let _local_fixture = local_fixture.await;
    let request = get_legacy_request(to, from);
    let response: LambdaResponse<ErrorFromHttpHandler> = fixture
        .lambda
        .invoke(FUNCTION_NAME, json!([{ "body": request }]))
        .await
        .unwrap_or_else(|_| panic!("There was an error invoking {FUNCTION_NAME}"));

    assert_error_from_http_handler(response, "Invalid H160");
}
