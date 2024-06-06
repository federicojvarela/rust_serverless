use crate::config::ConfigLoader;
use crate::fixtures::config::LambdaConfig;
use crate::helpers::lambda::LambdaClient;
use mpc_signature_sm::config::GlobalConfig;
use rstest::fixture;

pub struct LambdaFixture {
    pub config: LambdaConfig,
    pub localstack_url: String,
    pub lambda: LambdaClient,
}

#[fixture]
#[once]
pub fn fixture() -> LambdaFixture {
    let config = ConfigLoader::load_test::<LambdaConfig>();

    let localstack_url = ConfigLoader::load_test::<GlobalConfig>()
        .localstack_test_mode_endpoint
        .unwrap_or_else(|| panic!("unable to load lambda fixture, localstack url not configured."));

    let lambda = LambdaClient::new(config.lambda_watch_url.clone());

    LambdaFixture {
        config,
        localstack_url,
        lambda,
    }
}
