pub mod in_memory;

use crate::{config::GlobalConfig, feature_flags::in_memory::InMemoryFeatureFlagService};
use ana_tools::config_loader::ConfigLoader;
use ana_tools::feature_flags::FeatureFlagService;
use ana_tools::feature_flags::FeatureFlagServiceImpl;
use secrets_provider::SecretsProvider;
use std::sync::Arc;

const LAUNCHDARKLY_CONTEXT_KEY: &str = "mpc-signature-sm";
const FLAG_VERBOSE_LOG_MODE: &str = "mpc-verbose-log-mode";
pub const FLAG_ENABLE_SPONSORED_TRANSACTION: &str = "mpc-enable-sponsored-transaction";

pub struct FeatureFlags {
    service: Arc<dyn FeatureFlagService + Send + Sync>,
}

#[allow(unused)]
impl FeatureFlags {
    pub async fn new(secrets_provider: impl SecretsProvider) -> Self {
        let config = ConfigLoader::load_default::<GlobalConfig>();

        let launchdarkly_sdk_key_secret_name = config
            .launchdarkly_sdk_key_secret_name
            .as_ref()
            .expect("Env var missing: LAUNCHDARKLY_SDK_KEY_SECRET_NAME.");

        if config.feature_flag_in_memory_mode {
            return FeatureFlags::default_in_memory();
        }

        let sdk_key = secrets_provider
            .find(launchdarkly_sdk_key_secret_name)
            .await
            .expect("There was an error connecting to the secrets manager")
            .expect("Launch Darkly SDK KET was not present");

        let service =
            FeatureFlagServiceImpl::new(sdk_key.reveal(), LAUNCHDARKLY_CONTEXT_KEY.to_owned())
                .await
                .expect("Failed to created FeatureFlagService");

        Self {
            service: Arc::new(service),
        }
    }
    /// Only for local testing.
    pub fn new_from_in_memory(service: InMemoryFeatureFlagService) -> Self {
        Self {
            service: Arc::new(service),
        }
    }
    /// Only for local testing.
    pub fn default_in_memory() -> Self {
        FeatureFlags::new_from_in_memory(InMemoryFeatureFlagService::new())
    }
}

impl FeatureFlags {
    pub fn get_verbose_mode_flag(&self) -> bool {
        let default = false;
        self.service
            .get_flag_value(
                FLAG_VERBOSE_LOG_MODE,
                Some(LAUNCHDARKLY_CONTEXT_KEY.to_owned()),
                default.into(),
            )
            .as_bool()
            .unwrap_or(default)
    }

    pub fn get_enable_sponsored_transaction_flag(&self, client_id: &String) -> bool {
        let default = true;
        self.service
            .get_flag_value(
                FLAG_ENABLE_SPONSORED_TRANSACTION,
                Some(client_id.to_string()),
                default.into(),
            )
            .as_bool()
            .unwrap_or(default)
    }
}
