use std::str::FromStr;
use std::{sync::Arc, time::Duration};

use ana_tools::config_loader::ConfigLoader;
use anyhow::anyhow;
use aws_lambda_events::event::sqs::SqsMessageObj;
use ethers::utils::keccak256;
use k256::ecdsa::signature::hazmat::PrehashSigner;
use k256::ecdsa::SigningKey;
use k256::pkcs8::DecodePrivateKey;
use lambda_runtime::{Error, LambdaEvent};
use openssl::base64;
use rusoto_sqs::{SendMessageRequest, Sqs};
use secrets_provider::SecretsProvider;
use serde_json::json;
use tracing::level_filters::LevelFilter;
use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
use tracing_log::LogTracer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use common::aws_clients::secrets_manager::get_secrets_provider;
use common::aws_clients::sqs::get_sqs_client;
use config::Config;
use dtos::requests::ApproversRequest;
use mpc_signature_sm::result::error::LambdaError;

use crate::config::AutoApproverResult;

mod config;
mod dtos;

const REASON: &str = "This is an auto-approved transaction";

type SqsClientObject = Arc<dyn Sqs + Sync + Send>;

pub struct Persisted {
    pub config: Config,
    pub private_key: SigningKey,
    pub sqs_client: SqsClientObject,
}

type InputBody = Vec<SqsMessageObj<ApproversRequest>>;

// This lambda intentionally does not use the Lambda trait since it serves as an example of
// a lambda that a client would implement and that is not part of the orchestration service
#[tokio::main]
async fn main() -> Result<(), Error> {
    LogTracer::init()?;
    let app_name = concat!(env!("CARGO_PKG_NAME"), "-", env!("CARGO_PKG_VERSION")).to_string();
    let (non_blocking_writer, _guard) = tracing_appender::non_blocking(std::io::stdout());
    let bunyan_formatting_layer =
        BunyanFormattingLayer::new(app_name.to_string(), non_blocking_writer);

    tracing_subscriber::registry()
        .with(LevelFilter::WARN)
        .with(JsonStorageLayer)
        .with(bunyan_formatting_layer)
        .init();

    let config = ConfigLoader::load_default::<Config>();
    let sqs_client = Arc::new(get_sqs_client());
    let secrets_provider = get_secrets_provider().await;
    let private_key: String = secrets_provider
        .find(config.approver_private_key_secret_name.as_str())
        .await
        .expect("There was an error retrieving approver PK secret")
        .expect("Approver PK secret was not present in Secrets Manager")
        .reveal();
    let private_key =
        SigningKey::from_pkcs8_pem(&private_key).expect("Failed to parse Approver private key");

    let state = Arc::new(Persisted {
        config,
        private_key,
        sqs_client,
    });

    let service = |event: LambdaEvent<InputBody>| async { handler(event, state.clone()).await };

    lambda_runtime::run(lambda_runtime::service_fn(service)).await
}

async fn handler(event: LambdaEvent<InputBody>, state: Arc<Persisted>) -> Result<(), LambdaError> {
    let LambdaEvent { payload, context } = event;

    tracing::info!(payload = ?payload, context = ?context, "Execution started");

    // Spawn tokio tasks to process all the requests concurrently. Spawning a tasks sends it to
    // the tokio scheduler where it executes when it has work to do.
    // We send them all, so they start executing and evaluate their result after.
    let tasks = payload.into_iter().map(|request| {
        let message_id = request.message_id.unwrap_or_default();
        tracing::info!(
            order_id = ?request.body.contextual_data.order_id,
            message_id = message_id.clone(),
            "Calling default approver for order id {} from message id {}",
            request.body.contextual_data.order_id,
            message_id,
        );

        tokio::spawn(request_handler(
            request.body,
            state.config.clone(),
            state.private_key.clone(),
            state.sqs_client.clone(),
        ))
    });

    // Await them all
    for task in tasks {
        match task.await {
            Ok(Ok(_)) => (),
            Ok(Err(e)) => {
                tracing::error!(error = ?e, "there was an error processing an approver request.{e:?}");
                return Err(e);
            }
            Err(e) => {
                tracing::error!(error = ?e, "there was an error processing an approver request.{e:?}");
                return Err(LambdaError::Unknown(anyhow!(e).context(
                    "There was an error processing approver requests. Please check the logs",
                )));
            }
        }
    }

    Ok(())
}

/// Process an individual request
pub async fn request_handler(
    request: ApproversRequest,
    config: Config,
    private_key: SigningKey,
    sqs_client: SqsClientObject,
) -> Result<(), LambdaError> {
    // (1 = approved, 0 = rejected)
    // use the approval_status if provided. otherwise used the default based on the env variable.
    let approval_status: i32 = request.approval_status.unwrap_or_else(|| {
        let auto_approver_result = ConfigLoader::load_default::<Config>().auto_approver_result;
        AutoApproverResult::from_str(&auto_approver_result)
            .unwrap_or(AutoApproverResult::Approve)
            .into()
    });

    let bytes = request.transaction.as_rlp().map_err(|e| {
        LambdaError::Unknown(anyhow!(e).context("Error generating transaction bytes"))
    })?;

    let transaction_hash = keccak256(bytes);

    let status_reason = format!("{} from approver: {}", REASON, config.approver_name.clone());
    let metadata = base64::encode_block(
        serde_json::to_vec(&json!({
            "order_id": request.contextual_data.order_id,
            "transaction_hash": transaction_hash,
            "approval_status": approval_status,
            "status_reason": status_reason.clone()
        }))
        .map_err(|e| {
            LambdaError::Unknown(anyhow!(e).context("Error Base64 encoding approver metadata"))
        })?
        .as_slice(),
    );

    let (metadata_signature, _) = private_key
        .sign_prehash(&keccak256(metadata.as_bytes()))
        .map_err(|e| LambdaError::Unknown(anyhow!(e).context("Error signing approver metadata")))?;
    let metadata_signature = base64::encode_block(metadata_signature.to_der().as_bytes());
    let approver_name = config.approver_name.clone();

    let response = json!({
            "approver_name": approver_name,
            "order_id": request.contextual_data.order_id,
            "status_reason": status_reason.clone(),
            "approval_status": approval_status,
            "metadata": metadata,
            "metadata_signature": metadata_signature
    });

    tokio::time::sleep(Duration::from_secs(config.send_sqs_message_wait_seconds)).await;

    sqs_client
        .send_message(SendMessageRequest {
            message_body: response.to_string(),
            queue_url: config.response_queue_url.clone(),
            ..SendMessageRequest::default()
        })
        .await
        .map_err(|e| LambdaError::Unknown(anyhow!("Unable to send response to SQS: {e}")))?;

    tracing::info!(
        order_id = ?request.contextual_data.order_id,
        "Message published for order id {}",
        request.contextual_data.order_id,
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;
    use std::sync::Arc;

    use aws_lambda_events::sqs::SqsMessageObj;
    use ethers::types::transaction::eip712::TypedData;
    use ethers::types::H160;
    use ethers::utils::keccak256;
    use k256::ecdsa;
    use k256::ecdsa::signature::hazmat::PrehashSigner;
    use k256::ecdsa::SigningKey;
    use k256::pkcs8::DecodePrivateKey;
    use lambda_runtime::{Context, LambdaEvent};
    use mockall::predicate;
    use openssl::base64;
    use rstest::*;
    use rusoto_core::RusotoError;
    use rusoto_sqs::*;
    use serde_json::json;
    use uuid::Uuid;

    use common::test_tools::http::constants::ADDRESS_FOR_MOCK_REQUESTS;
    use common::test_tools::mocks::sqs_client::MockSqsClient;
    use model::order::helpers::sponsored_typed_data;
    use mpc_signature_sm::dtos::requests::transaction_request::TransactionRequest;

    use crate::config::Config;
    use crate::dtos::requests::ContextualDataRequest;
    use crate::{handler, ApproversRequest};
    use crate::{Persisted, REASON};

    const PEM: &str = r#"
-----BEGIN PRIVATE KEY-----
MIGEAgEAMBAGByqGSM49AgEGBSuBBAAKBG0wawIBAQQgbUttnDBQtE3/aFxcvcAe
qn1mmlOOyCp6u3s8qda6NyWhRANCAARBIEzhzlN18n1EtVTYzw1pggyGLxOYXktw
dkw/Y1H8dfRpGDPu5Me8ITxzOH2IiAY6QUlGXDZslRinmX7mjZT9
-----END PRIVATE KEY-----
"#;
    const ADDRESS_FROM: &str = "0xFF6A5DB899FB29F67A224CDA089572C2BC5A7A5E";

    struct TestFixture {
        pub config: Config,
        pub private_key: SigningKey,
        pub sqs_client: MockSqsClient,
        pub reason: String,
    }

    fn build_legacy_tx(to: &str) -> TransactionRequest {
        TransactionRequest::Legacy {
            to: H160::from_str(to).unwrap(),
            gas: 30000000.into(),
            gas_price: 800000.into(),
            value: 100.into(),
            nonce: 15.into(),
            data: hex::decode("6406516041610651325106165165106516169610")
                .unwrap()
                .into(),
            chain_id: 11155111,
        }
    }

    fn build_sponsored_tx() -> TransactionRequest {
        let sponsored_typed_data = sponsored_typed_data();

        let typed_data: TypedData = serde_json::from_value(sponsored_typed_data).unwrap();

        TransactionRequest::Sponsored {
            chain_id: 11155111,
            typed_data,
        }
    }

    #[fixture]
    fn fixture() -> TestFixture {
        let approver_name = String::from("approver1");
        TestFixture {
            config: Config {
                approver_name: approver_name.clone(),
                aws_region: "us-west-2".to_owned(),
                response_queue_url: "some.queue.url".to_owned(),
                approver_private_key_secret_name: "APPROVER_PK_SECRET_NAME".to_owned(),
                send_sqs_message_wait_seconds: 0,
                auto_approver_result: "approve".to_owned(),
            },
            private_key: SigningKey::from_pkcs8_pem(PEM).unwrap(),
            sqs_client: MockSqsClient::new(),
            reason: format!("{} from approver: {}", REASON, approver_name),
        }
    }

    #[rstest]
    #[case::approved(ADDRESS_FOR_MOCK_REQUESTS, 1)]
    #[tokio::test]
    async fn legacy_tx_successful_response(
        #[allow(unused_variables)]
        #[case]
        to: &str,
        #[case] approval_status: u32,
        mut fixture: TestFixture,
    ) {
        let order_id = Uuid::new_v4();
        let transaction = build_legacy_tx(to);
        let input = build_input_from(order_id, &transaction);

        let expected_response_queue_url = fixture.config.response_queue_url.clone();
        let expected_pk = fixture.private_key.clone();
        let approver_name = fixture.config.approver_name.clone();

        fixture
            .sqs_client
            .expect_send_message()
            .with(predicate::function(move |request: &SendMessageRequest| {
                let metadata = base64::encode_block(
                    serde_json::to_vec(&json!({
                        "order_id": order_id,
                        "transaction_hash": keccak256(transaction.as_rlp().unwrap()),
                        "approval_status": approval_status,
                        "status_reason": fixture.reason.clone()
                    }))
                    .unwrap()
                    .as_slice(),
                );
                let metadata_signature: ecdsa::Signature = expected_pk
                    .sign_prehash(&keccak256(metadata.as_bytes()))
                    .unwrap();
                let metadata_signature =
                    base64::encode_block(metadata_signature.to_der().as_bytes());

                let body = json!({
                    "approver_name": approver_name,
                    "order_id": order_id,
                    "status_reason": fixture.reason.clone(),
                    "approval_status": approval_status,
                    "metadata": metadata,
                    "metadata_signature": metadata_signature
                })
                .to_string();

                request.message_body == body && request.queue_url == expected_response_queue_url
            }))
            .times(1)
            .returning(move |_| Ok(SendMessageResult::default()));

        handler(
            LambdaEvent::new(input, Context::default()),
            Arc::new(Persisted {
                sqs_client: Arc::new(fixture.sqs_client),
                config: fixture.config,
                private_key: fixture.private_key,
            }),
        )
        .await
        .expect("Should succeed");
    }

    #[rstest]
    #[case::approved(1)]
    #[tokio::test]
    async fn sponsored_tx_successful_response(
        #[allow(unused_variables)]
        #[case]
        approval_status: u32,
        mut fixture: TestFixture,
    ) {
        let order_id = Uuid::new_v4();
        let transaction = build_sponsored_tx();
        let input = build_input_from(order_id, &transaction);

        let expected_response_queue_url = fixture.config.response_queue_url.clone();
        let expected_pk = fixture.private_key.clone();
        let approver_name = fixture.config.approver_name.clone();

        fixture
            .sqs_client
            .expect_send_message()
            .with(predicate::function(move |request: &SendMessageRequest| {
                let metadata = base64::encode_block(
                    serde_json::to_vec(&json!({
                        "order_id": order_id,
                        "transaction_hash": keccak256(transaction.as_rlp().unwrap()),
                        "approval_status": approval_status,
                        "status_reason": fixture.reason.clone()
                    }))
                    .unwrap()
                    .as_slice(),
                );
                let metadata_signature: ecdsa::Signature = expected_pk
                    .sign_prehash(&keccak256(metadata.as_bytes()))
                    .unwrap();
                let metadata_signature =
                    base64::encode_block(metadata_signature.to_der().as_bytes());

                let body = json!({
                    "approver_name": approver_name,
                    "order_id": order_id,
                    "status_reason": fixture.reason.clone(),
                    "approval_status": approval_status,
                    "metadata": metadata,
                    "metadata_signature": metadata_signature
                })
                .to_string();

                request.message_body == body && request.queue_url == expected_response_queue_url
            }))
            .times(1)
            .returning(move |_| Ok(SendMessageResult::default()));

        handler(
            LambdaEvent::new(input, Context::default()),
            Arc::new(Persisted {
                sqs_client: Arc::new(fixture.sqs_client),
                config: fixture.config,
                private_key: fixture.private_key,
            }),
        )
        .await
        .expect("Should succeed");
    }

    #[rstest]
    #[tokio::test]
    async fn fail_to_send_sqs_message(mut fixture: TestFixture) {
        let order_id = Uuid::new_v4();
        let tx = build_legacy_tx(ADDRESS_FOR_MOCK_REQUESTS);

        fixture
            .sqs_client
            .expect_send_message()
            .times(1)
            .returning(move |_| {
                Err(RusotoError::Service(
                    SendMessageError::InvalidMessageContents("invalid_fields".to_owned()),
                ))
            });

        let error = handler(
            LambdaEvent::new(build_input_from(order_id, &tx), Context::default()),
            Arc::new(Persisted {
                sqs_client: Arc::new(fixture.sqs_client),
                config: fixture.config,
                private_key: fixture.private_key,
            }),
        )
        .await
        .unwrap_err();

        assert!(error
            .to_string()
            .contains("Unable to send response to SQS: invalid_fields"));
    }

    fn build_input_from(
        order_id: Uuid,
        transaction_request: &TransactionRequest,
    ) -> Vec<SqsMessageObj<ApproversRequest>> {
        let body = ApproversRequest {
            contextual_data: ContextualDataRequest { order_id },
            transaction: transaction_request.clone(),
            from: H160::from_str(ADDRESS_FROM).unwrap(),
            approval_status: None,
        };

        let input = SqsMessageObj {
            message_id: None,
            receipt_handle: None,
            body,
            md5_of_body: None,
            md5_of_message_attributes: None,
            attributes: Default::default(),
            message_attributes: Default::default(),
            event_source_arn: None,
            event_source: None,
            aws_region: None,
        };

        vec![input]
    }
}
