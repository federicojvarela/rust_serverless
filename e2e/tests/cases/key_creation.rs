use crate::tools::fixtures::e2e::*;
use crate::tools::helper::*;
use crate::tools::runner::run_e2e_test;
use reqwest::StatusCode;
use rstest::rstest;
use serde_json::json;

// ⚠️⚠️⚠️ println! should be replaced with report library in the future ⚠️⚠️⚠️

#[rstest]
#[tokio::test(flavor = "multi_thread")]
async fn e2e_test_create_key_with_no_auth(e2e_fixture: &E2EFixture) {
    run_e2e_test(
        "No Auth",
        |test_context| async move {
            let config = test_context.config;
            let reqwest_client = test_context.client;

            print_with_time(
                "[+] [Create Key] - Creates a new key to sign transactions".to_string(),
            );

            let response = post_without_retry(
                &reqwest_client,
                &format!("{}/api/v1/keys", config.env_url),
                "",
                json!({
                    "client_user_id": config.client_user_id
                })
                .to_string(),
                "application/json",
            )
            .await;

            print_with_time("[=] [Create Key] - Assert Response".to_string());
            assert_eq!(StatusCode::UNAUTHORIZED, response.status());
        },
        e2e_fixture,
    )
    .await;
}
