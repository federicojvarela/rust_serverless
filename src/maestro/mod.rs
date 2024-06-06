pub mod config;
pub mod dtos;
pub mod session;
pub mod state;

use self::{
    config::MaestroConfig,
    session::{login, MaestroLoginInformation},
    state::MaestroState,
};
use crate::rest::middlewares::AuthenticationMiddleware;
use crate::result::error::Result;
use ana_tools::config_loader::ConfigLoader;
use secrets_provider::SecretsProvider;
use std::sync::Arc;

pub async fn maestro_bootstrap(secrets_provider: impl SecretsProvider) -> Result<MaestroState> {
    let maestro_config = ConfigLoader::load_default::<MaestroConfig>();

    let maestro_api_key = secrets_provider
        .find(&maestro_config.maestro_api_key_secret_name)
        .await
        .expect("Could not retrieve Maestro API Key secret from AWS Secrets Manager.")
        .expect("Maestro API Key secret not found")
        .reveal();

    let login_information = Arc::new(MaestroLoginInformation {
        maestro_url: maestro_config.maestro_url.clone(),
        service_name: maestro_config.service_name.clone(),
        maestro_api_key,
        tenant_name: maestro_config.maestro_tenant_name.clone(),
    });

    // TODO: We need to do this here because if we do not send an authorization header the
    // request will fail with 400 instead of 401 and will not be handled by the middleware. If
    // bolt solves this remove this line!
    let new_token = login(login_information.clone()).await?;

    let http_client = reqwest_middleware::ClientBuilder::new(reqwest::Client::new())
        .with(AuthenticationMiddleware::new(
            &login,
            login_information,
            Some(new_token),
        ))
        .build();

    Ok(MaestroState {
        http: http_client,
        config: maestro_config,
    })
}
