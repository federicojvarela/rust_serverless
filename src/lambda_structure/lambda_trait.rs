use async_trait::async_trait;
use lambda_runtime::{Error, LambdaEvent};
use serde::{de::DeserializeOwned, Serialize};
use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
use tracing_log::LogTracer;
use tracing_subscriber::{filter::LevelFilter, prelude::*, reload};

#[async_trait]
pub trait Lambda {
    type PersistedMemory: Sync + Send;
    type InputBody: DeserializeOwned + Send + Sync + std::fmt::Debug;
    type Output: Serialize + Send + Sync;
    type Error: Into<Error> + std::error::Error + Sync + Send + 'static;

    /// This function should be implemented to return any common connections or state that we want to persist between lambda executions.
    async fn bootstrap() -> Result<Self::PersistedMemory, Self::Error>;

    /// This function should be implemented with the actual business logic of the lambda.
    async fn run(
        payload: Self::InputBody,
        connections: &Self::PersistedMemory,
    ) -> Result<Self::Output, Self::Error>;

    /// A pre-configured main function that will bootstrap an instance of this lambda and start execution. Call this from the top-level main function for a given lambda.
    async fn main() -> Result<(), Error> {
        LogTracer::init()?;
        let app_name = concat!(env!("CARGO_PKG_NAME"), "-", env!("CARGO_PKG_VERSION")).to_string();
        let (non_blocking_writer, _guard) = tracing_appender::non_blocking(std::io::stdout());
        let bunyan_formatting_layer =
            BunyanFormattingLayer::new(app_name.to_string(), non_blocking_writer);

        // Instantiate a tracing subscriber with reloadable level filter
        let (filter, reload_handle) = reload::Layer::new(LevelFilter::WARN);
        tracing_subscriber::registry()
            .with(filter)
            .with(JsonStorageLayer)
            .with(bunyan_formatting_layer)
            .init();

        let reload_handle = &reload_handle;

        // Get a reference to avoid moving the original connections into the below closure.
        let persisted = &Self::bootstrap().await?;

        // Wrap our actual service call so we can pass in our connection data while preserving the expected Lambda signature.
        let service = move |event: LambdaEvent<Self::InputBody>| async move {
            reload_handle
                .modify(|filter| *filter = LevelFilter::WARN)
                .unwrap_or_else(|e| tracing::error!(error= ?e, "{:?}", e));

            Self::service(event, persisted).await
        };

        lambda_runtime::run(lambda_runtime::service_fn(service)).await
    }

    /// Service function that is called everytime the lambda executes. This includes common logic to pass our event context data through each lambda call.
    /// In the event of an error while executing a lambda, the original error will be mapped to also include our event context data.
    async fn service(
        event: LambdaEvent<Self::InputBody>,
        connections: &Self::PersistedMemory,
    ) -> Result<Self::Output, Self::Error> {
        let LambdaEvent { payload, context } = event;

        tracing::info!(payload = ?payload, context = ?context, "Execution started");

        // Call operation.
        Self::run(payload, connections).await
    }
}

#[macro_export]
macro_rules! lambda_main {
    ($lambda: ty) => {
        #[tokio::main]
        async fn main() -> $crate::result::error::LambdaRuntimeResult {
            use $crate::lambda_structure::lambda_trait::Lambda;
            <$lambda>::main().await
        }
    };
}
