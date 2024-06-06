use serde::Deserialize;

#[derive(Deserialize, Clone, Debug)]
pub struct MaestroConfig {
    /// Url for the maestro service.
    pub maestro_url: String,

    /// Service name inside maestro.
    pub service_name: String,

    /// Secret name holding Maestro's API key.
    pub maestro_api_key_secret_name: String,

    /// Tenant name used to log in
    pub maestro_tenant_name: String,
}
