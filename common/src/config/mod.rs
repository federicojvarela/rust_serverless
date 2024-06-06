pub mod aws_client_config;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use strum::IntoEnumIterator;
use strum::{Display, EnumIter};

// use common::models::Environment;
#[derive(Default, Serialize, Deserialize, Clone, Eq, PartialEq, EnumIter, Display)]
#[serde(rename_all = "lowercase")]
pub enum Environment {
    Local,
    #[default]
    Development,
    QA,
    Staging,
    Production,
}

pub struct ConfigLoader;

impl ConfigLoader {
    /// Loads the test configuration for the project. This is used
    /// for unit and integration tests.
    ///
    /// This will load the following files, in order:
    ///  - OS environment variables
    ///  - .env.test.local
    ///  - .env.test
    ///  - .env.local
    ///  - .env
    ///
    /// Variables are not overriden, the first file to contain
    /// a definition for a variable is the one that will be set.
    ///
    /// If a variable is set in the OS environment, it will not be
    /// overriden by any file.
    pub async fn load_test<TConfig>() -> TConfig
    where
        TConfig: DeserializeOwned,
    {
        dotenv::from_filename(".env.test.local").ok();
        dotenv::from_filename(".env.test").ok();
        ConfigLoader::load::<TConfig>().await
    }

    /// Loads the default configuration for the project. This is the
    /// configuration used in production.
    ///
    /// This will load the following files, in order:
    /// - OS environment variables
    /// - `.env.development` then `.env.development.local`
    /// - `.env.qa` then `.env.qa.local`
    /// - `.env.staging` then `.env.staging.local`
    /// - `.env.production` then `.env.production.local`
    /// - `.env.local`
    /// - `.env`
    ///
    /// Variables are not overriden, the first file to contain
    /// a definition for a variable is the one that will be set.
    ///
    /// If a variable is set in the OS environment, it will not be
    /// overriden by any file.
    pub async fn load_default<TConfig>() -> TConfig
    where
        TConfig: DeserializeOwned,
    {
        for environment in Environment::iter() {
            if environment != Environment::Local {
                dotenv::from_filename(format!(".env.{}.local", environment)).ok();
                dotenv::from_filename(format!(".env.{}", environment)).ok();
            }
        }

        ConfigLoader::load::<TConfig>().await
    }

    async fn load<TConfig>() -> TConfig
    where
        TConfig: DeserializeOwned,
    {
        dotenv::from_filename(".env.local").ok();
        dotenv::from_filename(".env").ok();

        envy::from_env::<TConfig>().expect("Could not load configuration")
    }
}
