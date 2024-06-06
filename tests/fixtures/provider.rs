use ana_tools::config_loader::ConfigLoader;
use ethers::providers::{Http, Provider};
use rstest::fixture;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Config {
    #[serde(rename(deserialize = "ethereum_mainnet_endpoint"))]
    /// All endpoint env vars point to ganache
    pub ganache_endpoint: String,
}

pub struct ProviderFixture {
    pub provider: Provider<Http>,
}

#[fixture]
#[once]
pub fn provider_fixture() -> ProviderFixture {
    let config = ConfigLoader::load_test::<Config>();
    let ganache_endpoint = config.ganache_endpoint;
    let provider = Provider::<Http>::try_from(ganache_endpoint).unwrap();

    ProviderFixture { provider }
}
