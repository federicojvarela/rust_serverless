mod config;
mod dtos;

use async_trait::async_trait;
use common::aws_clients::secrets_manager::get_secrets_provider;
use ethers::prelude::U256;
use mpc_signature_sm::blockchain::providers::alchemy::alchemy_blockchain_provider::AlchemyEvmBlockchainProvider;
use mpc_signature_sm::blockchain::providers::EvmBlockchainProvider;
use repositories::cache::cache_repository_impl::CacheRepositoryImpl;
use repositories::nonces::nonces_repository_impl::NoncesRepositoryImpl;
use repositories::nonces::{NoncesRepository, NoncesRepositoryError};
use std::sync::Arc;

use crate::config::Config;
use crate::dtos::{Request, Response};
use ana_tools::config_loader::ConfigLoader;
use common::aws_clients::dynamodb::get_dynamodb_client;
use mpc_signature_sm::result::error::LambdaError;
use mpc_signature_sm::{lambda_main, lambda_structure::lambda_trait::Lambda};

type BlockchainProviderObject = Arc<dyn EvmBlockchainProvider + Sync + Send>;

pub struct Persisted {
    pub config: Config,
    pub nonces_repository: Arc<dyn NoncesRepository>,
    pub blockchain_provider: BlockchainProviderObject,
}
pub struct AdminUpdateAddressNonce;

#[async_trait]
impl Lambda for AdminUpdateAddressNonce {
    type PersistedMemory = Persisted;
    type InputBody = Request;
    type Output = Response;
    type Error = LambdaError;

    async fn bootstrap() -> Result<Self::PersistedMemory, Self::Error> {
        let config = ConfigLoader::load_default::<Config>();
        let dynamodb_client = get_dynamodb_client();
        let nonces_repository = Arc::new(NoncesRepositoryImpl::new(
            config.nonces_table_name.clone(),
            dynamodb_client.clone(),
        )) as Arc<dyn NoncesRepository>;

        let blockchain_config =
            ConfigLoader::load_default::<mpc_signature_sm::blockchain::config::Config>();

        let cache_repository = Arc::new(CacheRepositoryImpl::new(
            config.cache_table_name.clone(),
            dynamodb_client.clone(),
        ));

        let secrets_provider = get_secrets_provider().await;
        let blockchain_provider = Arc::new(AlchemyEvmBlockchainProvider::new(
            blockchain_config,
            secrets_provider,
            cache_repository,
        )) as BlockchainProviderObject;

        Ok(Persisted {
            config,
            nonces_repository,
            blockchain_provider,
        })
    }

    async fn run(
        request: Self::InputBody,
        state: &Self::PersistedMemory,
    ) -> Result<Self::Output, Self::Error> {
        let current_nonce = match state
            .nonces_repository
            .get_nonce(request.address, request.chain_id)
            .await
        {
            Ok(nonce) => Ok(nonce.nonce),
            Err(e) => match e {
                NoncesRepositoryError::NonceNotFound(_) => Ok(U256::from(0)),
                e => Err(LambdaError::Unknown(anyhow::anyhow!(format!(
                    "There was an error getting the nonce for address {} and chain id {}.{e:?}",
                    request.address, request.chain_id
                )))),
            },
        }?;

        tracing::info!(
            address = ?request.address,
            request.chain_id,
            nonce = ?current_nonce,
            "Current DB nonce for address {:?} in chain {} is {}",
            request.address,
            request.chain_id,
            current_nonce
        );

        let nonce = state
            .blockchain_provider
            .get_current_nonce(request.chain_id, request.address)
            .await
            .map_err(|e| {
                LambdaError::Unknown(anyhow::anyhow!(
                    "There was an error getting the nonce for address {} and chain id {}.{e:?}",
                    format!("{:#0x}", request.address),
                    request.chain_id
                ))
            })?;

        tracing::info!(
            address = ?request.address,
            request.chain_id,
            nonce = ?nonce,
            "Current blockchain nonce for address {:?} in chain {} is {}",
            format!("{:#0x}", request.address),
            request.chain_id,
            nonce
        );

        state
            .nonces_repository
            .set_nonce(request.address, nonce, None, request.chain_id)
            .await
            .map_err(|e| {
                LambdaError::Unknown(anyhow::anyhow!(
                    "There was an error setting the nonce for address {} and chain id {}.{e:?}",
                    format!("{:#0x}", request.address),
                    request.chain_id
                ))
            })?;

        Ok(Response {
            old_nonce: current_nonce,
            new_nonce: nonce,
        })
    }
}

lambda_main!(AdminUpdateAddressNonce);

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use chrono::Utc;
    use common::test_tools::http::constants::ADDRESS_FOR_MOCK_REQUESTS;
    use ethers::{
        abi::Address,
        types::{Transaction, H160, U256},
    };
    use mockall::mock;
    use model::nonce::Nonce;
    use mpc_signature_sm::{
        blockchain::providers::{
            BlockFeeQuery, BlockchainProviderError, EvmBlockchainProvider, FeeHistory,
            FungibleTokenInfo, FungibleTokenMetadataInfo, NativeTokenInfo, NewestBlock,
            NonFungibleTokenInfo, Pagination,
        },
        lambda_structure::lambda_trait::Lambda,
    };
    use repositories::nonces::MockNoncesRepository;
    use repositories::nonces::NoncesRepositoryError;
    use rstest::{fixture, rstest};
    use std::{str::FromStr, sync::Arc};

    use crate::{config::Config, dtos::Request, AdminUpdateAddressNonce, Persisted};

    mock! {
        BlockchainProvider {}
        #[async_trait]
        impl EvmBlockchainProvider  for  BlockchainProvider {
            async fn get_evm_endpoint(
                &self,
                chain_id: u64,
                endpoint_prefix: Option<String>,
            ) -> Result<String, BlockchainProviderError>;

            async fn get_native_token_info(
                &self,
                chain_id: u64,
                address: Address,
            ) -> Result<NativeTokenInfo, BlockchainProviderError>;

            async fn get_non_fungible_token_info(
                &self,
                chain_id: u64,
                address: Address,
                contract_addresses: Vec<Address>,
                pagination: Pagination,
            ) -> Result<NonFungibleTokenInfo, BlockchainProviderError>;

            async fn get_fungible_token_info(
                &self,
                chain_id: u64,
                address: Address,
                contract_addresses: Vec<Address>,
            ) -> Result<FungibleTokenInfo, BlockchainProviderError>;

            async fn get_fungible_token_metadata(
                &self,
                chain_id: u64,
                address: Address,
            ) -> Result<FungibleTokenMetadataInfo, BlockchainProviderError>;

            async fn get_fee_history<'percentiles>(
                &self,
                chain_id: u64,
                block_count: u64,
                newest_block: NewestBlock,
                reward_percentiles: &'percentiles [f64],
            ) -> Result<FeeHistory, BlockchainProviderError>;

            async fn tx_status_succeed(
                &self,
                chain_id: u64,
                tx_hash: String,
            ) -> Result<bool, BlockchainProviderError>;

            async fn get_tx_by_hash(
                &self,
                chain_id: u64,
                tx_hash: String,
            ) -> Result<Option<Transaction>, BlockchainProviderError>;

            async fn get_tx_receipt(
                &self,
                chain_id: u64,
                tx_hash: String,
            ) -> Result<Option<ethers::types::TransactionReceipt>, BlockchainProviderError>;

            async fn get_current_nonce(
                &self,
                chain_id: u64,
                address: Address
            ) -> Result<U256, BlockchainProviderError>;

            async fn get_fees_from_pending(
                &self,
                chain_id: u64,
            ) -> Result<BlockFeeQuery, BlockchainProviderError>;
        }
    }

    struct TestFixture {
        pub config: Config,
        pub nonces_repository: MockNoncesRepository,
        pub blockchain_provider: MockBlockchainProvider,
    }

    #[fixture]
    fn fixture() -> TestFixture {
        TestFixture {
            config: Config {
                nonces_table_name: "nonces".to_string(),
                cache_table_name: "cache".to_owned(),
            },
            nonces_repository: MockNoncesRepository::new(),
            blockchain_provider: MockBlockchainProvider::new(),
        }
    }

    #[rstest]
    #[tokio::test]
    async fn admin_update_nonce_blockchain_provider_fails(mut fixture: TestFixture) {
        fixture
            .nonces_repository
            .expect_get_nonce()
            .once()
            .returning(|_, _| {
                Ok(Nonce {
                    nonce: U256::from(0),
                    chain_id: 1,
                    address: H160::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap(),
                    created_at: Utc::now(),
                    last_modified_at: Utc::now(),
                })
            });

        fixture
            .blockchain_provider
            .expect_get_current_nonce()
            .once()
            .returning(|_, _| Err(BlockchainProviderError::Unknown(anyhow::anyhow!("error!"))));

        let request = Request {
            address: H160::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap(),
            chain_id: 11155111,
        };

        let state = Persisted {
            config: fixture.config,
            nonces_repository: Arc::new(fixture.nonces_repository),
            blockchain_provider: Arc::new(fixture.blockchain_provider),
        };

        let result = AdminUpdateAddressNonce::run(request, &state).await;

        assert!(result.is_err())
    }

    #[rstest]
    #[tokio::test]
    async fn admin_update_nonce_dynamo_fails(mut fixture: TestFixture) {
        fixture
            .blockchain_provider
            .expect_get_current_nonce()
            .once()
            .returning(|_, _| Ok(U256::from(1)));

        fixture
            .nonces_repository
            .expect_get_nonce()
            .once()
            .returning(|_, _| {
                Ok(Nonce {
                    nonce: U256::from(0),
                    chain_id: 1,
                    address: H160::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap(),
                    created_at: Utc::now(),
                    last_modified_at: Utc::now(),
                })
            });

        fixture
            .nonces_repository
            .expect_set_nonce()
            .once()
            .returning(|_, _, _, _| Err(NoncesRepositoryError::Unknown(anyhow::anyhow!("error!"))));

        let request = Request {
            address: H160::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap(),
            chain_id: 11155111,
        };

        let state = Persisted {
            config: fixture.config,
            nonces_repository: Arc::new(fixture.nonces_repository),
            blockchain_provider: Arc::new(fixture.blockchain_provider),
        };

        let result = AdminUpdateAddressNonce::run(request, &state).await;

        assert!(result.is_err())
    }

    #[rstest]
    #[tokio::test]
    async fn admin_update_nonce_ok(mut fixture: TestFixture) {
        fixture
            .blockchain_provider
            .expect_get_current_nonce()
            .once()
            .returning(|_, _| Ok(U256::from(1)));

        fixture
            .nonces_repository
            .expect_get_nonce()
            .once()
            .returning(|_, _| {
                Ok(Nonce {
                    nonce: U256::from(0),
                    chain_id: 1,
                    address: H160::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap(),
                    created_at: Utc::now(),
                    last_modified_at: Utc::now(),
                })
            });

        fixture
            .nonces_repository
            .expect_set_nonce()
            .once()
            .returning(|_, _, _, _| Ok(()));

        let request = Request {
            address: H160::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap(),
            chain_id: 11155111,
        };

        let state = Persisted {
            config: fixture.config,
            nonces_repository: Arc::new(fixture.nonces_repository),
            blockchain_provider: Arc::new(fixture.blockchain_provider),
        };

        let response = AdminUpdateAddressNonce::run(request, &state).await.unwrap();

        assert_eq!(response.old_nonce, U256::from(0));
        assert_eq!(response.new_nonce, U256::from(1));
    }
}
