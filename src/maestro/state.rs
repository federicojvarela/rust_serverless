use super::config::MaestroConfig;
use reqwest_middleware::ClientWithMiddleware;

#[derive(Debug)]
pub struct MaestroState {
    pub http: ClientWithMiddleware,
    pub config: MaestroConfig,
}
