mod config;
mod dtos;
use crate::config::Config;
use ana_tools::config_loader::ConfigLoader;
use anyhow::anyhow;
use async_trait::async_trait;
use common::aws_clients::dynamodb::get_dynamodb_client;
use dtos::ChainListenerEvent;
use mpc_signature_sm::validations::address::address_validator::AddressValidatorImpl;
use mpc_signature_sm::validations::address::AddressValidator;
use mpc_signature_sm::{
    lambda_main, lambda_structure::lambda_trait::Lambda, result::error::LambdaError,
};
use repositories::keys::keys_repository_impl::KeysRepositoryImpl;
use repositories::nonces::nonces_repository_impl::NoncesRepositoryImpl;
use repositories::nonces::NoncesRepository;
use std::sync::Arc;

type AddressValidatorObject = Arc<dyn AddressValidator + Sync + Send>;

pub struct Persisted {
    pub nonces_repository: Arc<dyn NoncesRepository>,
    pub address_validator: AddressValidatorObject,
}

pub struct MpcNonceWriter;

#[async_trait]
impl Lambda for MpcNonceWriter {
    type PersistedMemory = Persisted;
    type InputBody = ChainListenerEvent;
    type Output = ();
    type Error = LambdaError;

    async fn bootstrap() -> Result<Self::PersistedMemory, Self::Error> {
        let config = ConfigLoader::load_default::<Config>();
        let dynamo_db_client = get_dynamodb_client();
        let address_validator = Arc::new(AddressValidatorImpl::new(KeysRepositoryImpl::new(
            config.keys_table_name.clone(),
            dynamo_db_client.clone(),
        )));
        let nonces_repository = Arc::new(NoncesRepositoryImpl::new(
            config.nonces_table_name,
            dynamo_db_client,
        ));

        Ok(Persisted {
            nonces_repository,
            address_validator,
        })
    }

    async fn run(
        request: Self::InputBody,
        state: &Self::PersistedMemory,
    ) -> Result<Self::Output, Self::Error> {
        // Check if the address should be handled
        let valid = state
            .address_validator
            .valid_from_address(format!("{:?}", request.detail.from))
            .await?;

        if !valid {
            // if we do not have the address in our DB we return without error
            // and without logging
            return Ok(());
        }

        tracing::info!(
            address = ?request.detail.from,
            nonce = ?request.detail.nonce,
            request.detail.hash,
            request.detail.chain_id,
            "Writing tx nonce {} to address {} and chain id {}",
            request.detail.nonce,
            request.detail.from,
            request.detail.chain_id
        );

        state
            .nonces_repository
            .increment_nonce(
                request.detail.from,
                request.detail.nonce,
                request.detail.hash,
                request.detail.chain_id,
            )
            .await
            .map_err(|e| LambdaError::Unknown(anyhow!(e)))?;

        Ok(())
    }
}

lambda_main!(MpcNonceWriter);

#[cfg(test)]
mod tests {
    use crate::dtos::{ChainListenerEvent, Transaction};
    use crate::{MpcNonceWriter, Persisted};
    use anyhow::anyhow;
    use async_trait::async_trait;
    use common::test_tools::http::constants::HASH_FOR_MOCK_REQUESTS;
    use ethers::types::{H160, U256};
    use mockall::{mock, predicate};
    use mpc_signature_sm::lambda_structure::lambda_trait::Lambda;
    use mpc_signature_sm::result::error::LambdaError;
    use mpc_signature_sm::validations::address::AddressValidator;
    use mpc_signature_sm::validations::address::AddressValidatorError;
    use repositories::nonces::NoncesRepositoryError;
    use repositories::nonces::*;
    use rstest::*;
    use std::sync::Arc;

    mock! {
        AddrValidator {}
        #[async_trait]
        impl AddressValidator for AddrValidator {
            async fn valid_from_address(&self, address: String) -> Result<bool, AddressValidatorError>;
        }
    }

    struct TestFixture {
        pub request: ChainListenerEvent,
        pub address_validator: MockAddrValidator,
        pub nonces_repository: MockNoncesRepository,
    }

    #[fixture]
    fn fixture() -> TestFixture {
        TestFixture {
            request: ChainListenerEvent {
                detail: Transaction {
                    from: H160::random(),
                    hash: HASH_FOR_MOCK_REQUESTS.to_string(),

                    nonce: U256::from(1),
                    chain_id: 1,
                },
            },
            address_validator: MockAddrValidator::new(),
            nonces_repository: MockNoncesRepository::new(),
        }
    }

    #[rstest]
    #[tokio::test]
    async fn nonce_writer_update_db_error(mut fixture: TestFixture) {
        fixture
            .address_validator
            .expect_valid_from_address()
            .once()
            .with(predicate::eq(format!("{:?}", fixture.request.detail.from)))
            .returning(|_| Ok(true));

        fixture
            .nonces_repository
            .expect_increment_nonce()
            .with(
                predicate::eq(fixture.request.detail.from),
                predicate::eq(fixture.request.detail.nonce),
                predicate::eq(fixture.request.detail.hash.clone()),
                predicate::eq(fixture.request.detail.chain_id),
            )
            .once()
            .returning(|_, _, _, _| Err(NoncesRepositoryError::Unknown(anyhow!("timeout!"))));

        let result = MpcNonceWriter::run(
            fixture.request,
            &Persisted {
                address_validator: Arc::new(fixture.address_validator),
                nonces_repository: Arc::new(fixture.nonces_repository),
            },
        )
        .await;
        assert!(result.is_err());
        let error = result.err().unwrap();
        assert!(matches!(error, LambdaError::Unknown(_)));
        assert!(error.to_string().contains("timeout!"));
    }

    #[rstest]
    #[tokio::test]
    async fn nonce_writer_updates_successfully(mut fixture: TestFixture) {
        fixture
            .address_validator
            .expect_valid_from_address()
            .once()
            .with(predicate::eq(format!("{:?}", fixture.request.detail.from)))
            .returning(|_| Ok(true));

        fixture
            .nonces_repository
            .expect_increment_nonce()
            .with(
                predicate::eq(fixture.request.detail.from),
                predicate::eq(fixture.request.detail.nonce),
                predicate::eq(fixture.request.detail.hash.clone()),
                predicate::eq(fixture.request.detail.chain_id),
            )
            .once()
            .returning(|_, _, _, _| Ok(()));

        MpcNonceWriter::run(
            fixture.request,
            &Persisted {
                address_validator: Arc::new(fixture.address_validator),
                nonces_repository: Arc::new(fixture.nonces_repository),
            },
        )
        .await
        .expect("should succeed");
    }

    #[rstest]
    #[tokio::test]
    async fn nonce_writer_with_not_handled_address(mut fixture: TestFixture) {
        fixture
            .address_validator
            .expect_valid_from_address()
            .once()
            .with(predicate::eq(format!("{:?}", fixture.request.detail.from)))
            .returning(|_| Ok(false));

        fixture.nonces_repository.expect_get_nonce().never();

        MpcNonceWriter::run(
            fixture.request,
            &Persisted {
                address_validator: Arc::new(fixture.address_validator),
                nonces_repository: Arc::new(fixture.nonces_repository),
            },
        )
        .await
        .expect("should succeed");
    }

    #[rstest]
    #[tokio::test]
    async fn nonce_writer_address_validation_error(mut fixture: TestFixture) {
        fixture
            .address_validator
            .expect_valid_from_address()
            .once()
            .with(predicate::eq(format!("{:?}", fixture.request.detail.from)))
            .returning(|_| Err(AddressValidatorError::Unknown(anyhow!("timeout!"))));

        fixture.nonces_repository.expect_increment_nonce().never();

        let result = MpcNonceWriter::run(
            fixture.request,
            &Persisted {
                address_validator: Arc::new(fixture.address_validator),
                nonces_repository: Arc::new(fixture.nonces_repository),
            },
        )
        .await;

        assert!(result.is_err());
        let error = result.err().unwrap();
        assert!(matches!(error, LambdaError::Unknown(_)));
        assert!(error.to_string().contains("timeout!"));
    }
}
