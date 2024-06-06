use crate::tools::fixtures::e2e::*;
use crate::tools::helper::*;
use crate::tools::runner::run_e2e_test;
use reqwest::StatusCode;
use rstest::rstest;
use uuid::Uuid;

// ⚠️⚠️⚠️ println! should be replaced with report library in the future ⚠️⚠️⚠️

#[rstest]
#[tokio::test(flavor = "multi_thread")]
async fn e2e_test_get_order_status_with_no_auth(e2e_fixture: &E2EFixture) {
    run_e2e_test(
        "No Auth",
        |test_context| async move {
            let config = test_context.config;
            let reqwest_client = test_context.client;

            let order_id = Uuid::new_v4().to_string();
            print_with_time(format!("[!] [Get order status] - OrderID: {}", order_id));

            let response = get_without_retry(
                &reqwest_client,
                &format!("{}/api/v1/orders/{}/status", config.env_url, order_id),
                "",
            )
            .await;

            print_with_time("[=] [Get order status] - Assert Response".to_string());
            assert_eq!(StatusCode::UNAUTHORIZED, response.status());
        },
        e2e_fixture,
    )
    .await;
}
