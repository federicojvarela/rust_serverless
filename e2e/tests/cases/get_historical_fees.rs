use crate::tools::fixtures::e2e::*;
use crate::tools::helper::*;
use rstest::rstest;

const END_POINT: &str = "Get historical fees";

#[rstest]
#[case::missing_auth_token("")]
#[case::invalid_auth_token("0")]
#[tokio::test(flavor = "multi_thread")]
async fn e2e_test_historical_fees_bad_auth_token(
    e2e_fixture: &E2EFixture,
    #[case] auth_token: &str,
) {
    let env = &e2e_fixture.config.environment;
    let test_group = "invalid auth_tokens";
    start_test(env, test_group);

    let config = &e2e_fixture.config;
    let reqwest_client = &e2e_fixture.client;

    print_with_time(format!(
        "[=] [{}] - Query historical fees - testing authorization_token {}",
        END_POINT, auth_token
    ));
    let response = get_historical_fees(
        config,
        reqwest_client,
        auth_token,
        "1", // some valid chain_id
        false,
    )
    .await;
    assert_eq!(response["message"], "Unauthorized");

    // }
    end_test(env, test_group);
}

fn start_test(env: &str, test_group: &str) {
    println!();
    print_with_time(format!(
        "[+] Starting Rust E2E {} - {} - on {} Environment",
        END_POINT, test_group, env
    ));
}
fn end_test(env: &str, test_group: &str) {
    print_with_time(format!(
        "[-] Finished Rust E2E {} - {} - on {} Environment",
        END_POINT, test_group, env
    ));
}
