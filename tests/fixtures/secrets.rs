use crate::config::ConfigLoader;
use mpc_signature_sm::config::GlobalConfig;
use rstest::fixture;
use rusoto_core::Region;
use rusoto_secretsmanager::SecretsManagerClient;

use common::block_on;

pub struct SecretsManagerFixture {
    pub secrets_manager: SecretsManagerClient,
}

#[fixture]
#[once]
pub fn secrets_manager_fixture() -> SecretsManagerFixture {
    let config = ConfigLoader::load_test::<GlobalConfig>();
    let endpoint = config
        .localstack_test_mode_endpoint
        .expect("Unable to create Secrets Manager client: localstack endpoint not present");

    let secrets_manager = SecretsManagerClient::new(Region::Custom {
        name: config.aws_region,
        endpoint,
    });

    block_on!(async { SecretsManagerFixture { secrets_manager } })
}
