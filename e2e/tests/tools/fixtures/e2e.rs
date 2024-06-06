use crate::tools::config::Config;
use rstest::fixture;
use std::sync::Arc;

#[derive(Clone)]
pub struct TestContext {
    pub config: Arc<Config>,
    pub chain_id: u64,
    pub client: Arc<reqwest::Client>,
}

pub struct E2EFixture {
    pub config: Arc<Config>,
    pub client: Arc<reqwest::Client>,
}

#[fixture]
#[once]
pub fn e2e_fixture() -> E2EFixture {
    let config = Config::load_test();
    let reqwest_client = reqwest::Client::new();

    E2EFixture {
        config: Arc::new(config),
        client: Arc::new(reqwest_client),
    }
}
