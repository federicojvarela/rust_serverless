mod config;
mod dtos;
mod models;

use crate::config::Config;
use ana_tools::config_loader::ConfigLoader;
use anyhow::anyhow;
use async_trait::async_trait;
use chrono::Utc;
use common::aws_clients::dynamodb::get_dynamodb_client;
use common::aws_clients::secrets_manager::get_secrets_provider;
use common::aws_clients::step_functions::get_step_functions_client;
use dtos::{TransactionBundlerRequest, TransactionBundlerResponse};
use ethers::abi::Abi;
use ethers::contract::Contract;
use ethers::providers::{Http, Provider};
use rusoto_stepfunctions::StepFunctions;
use serde_json::Value;
use std::sync::Arc;
use uuid::Uuid;

use model::order::{
    GenericOrderData, OrderState, OrderStatus, OrderTransaction, OrderType, SharedOrderData,
    SignatureOrderData,
};
use models::{SponsoredTransaction, U48};

use mpc_signature_sm::blockchain::gas_fees::prediction::get_predicted_fees;
use mpc_signature_sm::blockchain::providers::alchemy::alchemy_blockchain_provider::AlchemyEvmBlockchainProvider;
use mpc_signature_sm::blockchain::providers::EvmBlockchainProvider;
use mpc_signature_sm::dtos::requests::send_to_approvers_sm::SendToApproversStateMachineRequest;
use mpc_signature_sm::lambda_abstractions::invoke_step_function_async::{
    invoke_step_function_async_dyn_client, StepFunctionConfig,
};
use mpc_signature_sm::lambda_structure::event::Event;
use mpc_signature_sm::model::step_function::StepFunctionContext;
use mpc_signature_sm::{
    lambda_main, lambda_structure::lambda_trait::Lambda, result::error::OrchestrationError,
    result::Result,
};

use repositories::cache::cache_repository_impl::CacheRepositoryImpl;
use repositories::keys::keys_repository_impl::KeysRepositoryImpl;
use repositories::keys::{KeysRepository, KeysRepositoryError};
use repositories::orders::orders_repository_impl::OrdersRepositoryImpl;
use repositories::orders::OrdersRepository;

type BlockchainProviderObject = Arc<dyn EvmBlockchainProvider + Sync + Send>;
type StepFunctionsObject = Arc<dyn StepFunctions + Sync + Send>;

pub struct Persisted {
    pub config: Config,
    pub blockchain_provider: BlockchainProviderObject,
    pub orders_repository: Arc<dyn OrdersRepository>,
    pub keys_repository: Arc<dyn KeysRepository>,
    pub step_functions_client: StepFunctionsObject,
}

pub struct TransactionBundler;

#[async_trait]
impl Lambda for TransactionBundler {
    type PersistedMemory = Persisted;
    type InputBody = Event<TransactionBundlerRequest>;
    type Output = Event<TransactionBundlerResponse>;
    type Error = OrchestrationError;

    async fn bootstrap() -> Result<Self::PersistedMemory> {
        let config = ConfigLoader::load_default::<Config>();
        let dynamodb_client = get_dynamodb_client();

        let orders_repository = Arc::new(OrdersRepositoryImpl::new(
            config.order_status_table_name,
            dynamodb_client.clone(),
        )) as Arc<dyn OrdersRepository>;

        let dynamo_db_client = get_dynamodb_client();
        let secrets_provider = get_secrets_provider().await;
        let config = ConfigLoader::load_default::<Config>();
        let cache_repository = Arc::new(CacheRepositoryImpl::new(
            config.cache_table_name.clone(),
            dynamo_db_client.clone(),
        ));

        let keys_repository = Arc::new(KeysRepositoryImpl::new(
            config.keys_table_name.clone(),
            dynamodb_client,
        )) as Arc<dyn KeysRepository>;

        let blockchain_config =
            ConfigLoader::load_default::<mpc_signature_sm::blockchain::config::Config>();
        let blockchain_provider = Arc::new(AlchemyEvmBlockchainProvider::new(
            blockchain_config,
            secrets_provider,
            cache_repository,
        )) as BlockchainProviderObject;

        let step_functions_client: StepFunctionsObject = Arc::new(get_step_functions_client());

        Ok(Persisted {
            config,
            blockchain_provider,
            orders_repository,
            keys_repository,
            step_functions_client,
        })
    }

    async fn run(request: Self::InputBody, state: &Self::PersistedMemory) -> Result<Self::Output> {
        // 1. GET SPONSORED ORDER INFORMATION
        let sponsored_order = state
            .orders_repository
            .get_order_by_id(request.context.order_id.to_string())
            .await
            .map_err(|e| OrchestrationError::from(anyhow!(e)))?;

        if let Some(wrapper_order_id) = sponsored_order.replaced_by {
            return Ok(Event {
                payload: TransactionBundlerResponse {
                    order_id: wrapper_order_id,
                },
                context: request.context,
            });
        }

        let sponsored_order_data = sponsored_order.extract_signature_data().map_err(|e| {
            OrchestrationError::from(anyhow!(e).context(
                "there was an error extracting the signature data from the sponsored order",
            ))
        })?;

        let sponsored_transaction = sponsored_order_data.data.transaction;

        let (typed_data, chain_id, sponsor_addresses) = match sponsored_transaction {
            OrderTransaction::Sponsored {
                typed_data,
                chain_id,
                to: _,
                sponsor_addresses,
            } => (typed_data, chain_id, sponsor_addresses),
            _ => {
                return Err(OrchestrationError::from(anyhow!(
                    "Order was not of type SPONSORED"
                )))
            }
        };

        let forwarder_address =
            typed_data
                .domain
                .verifying_contract
                .ok_or(OrchestrationError::from(anyhow!(
                    "missing verifying contract"
                )))?;

        // 2. ENCODE SPONSORED TRANSACTION
        let endpoint = state
            .blockchain_provider
            .get_evm_endpoint(chain_id, None)
            .await
            .map_err(|e| OrchestrationError::from(anyhow!(e).context("Unable to get endpoint")))?;

        let provider = Provider::<Http>::try_from(endpoint).map_err(|e| {
            OrchestrationError::from(anyhow!(e).context("Unable to instantiate provider"))
        })?;

        let abi: Abi = serde_json::from_str(include_str!("./forwarder_abi.json")).map_err(|e| {
            OrchestrationError::from(anyhow!(e).context("Failed to get abi json file"))
        })?;

        let contract = Contract::new(forwarder_address, abi, provider.into());

        let json_value = Value::Object(typed_data.message.into_iter().collect());
        let transaction: SponsoredTransaction =
            serde_json::from_value(json_value).map_err(|e| OrchestrationError::from(anyhow!(e)))?;

        let signature =
            sponsored_order_data
                .data
                .maestro_signature
                .ok_or(OrchestrationError::from(anyhow!(
                    "Missing maestro signature"
                )))?;

        let deadline: U48 = transaction.deadline.try_into().map_err(|_| {
            OrchestrationError::from(anyhow!("failed to convert deadline u64 to u48"))
        })?;

        let batch = vec![(
            transaction.from,
            transaction.to,
            transaction.value,
            transaction.gas,
            deadline,
            transaction.data,
            signature,
        )];

        let from_address = sponsored_order_data.data.address;

        let encoded_data = contract
            .encode("executeBatch", (batch, from_address))
            .map_err(|e| {
                OrchestrationError::from(anyhow!(e).context("Failed to encode executeBatch call"))
            })?;

        // 3. GET GAS POOL INFORMATION
        let gas_pool_address = sponsor_addresses.gas_pool_address;

        // The gas pool is a WaaS account so there's a key entry for it
        let gas_pool_address_key = state
            .keys_repository
            .get_key_by_address(gas_pool_address)
            .await
            .map_err(|e| match e {
                KeysRepositoryError::Unknown(e) => OrchestrationError::from(anyhow!(e)),
                KeysRepositoryError::KeyNotFound(e) => {
                    OrchestrationError::from(anyhow!(e).context("Key not found for address"))
                }
            })?;

        // 4. GET PREDICTED GAS FEES
        let fees = get_predicted_fees(&*state.blockchain_provider, chain_id)
            .await
            .map_err(|e| {
                OrchestrationError::from(anyhow!(e).context("Failed to get predicted gas fees"))
            })?;

        let max_fee_per_gas = fees.max_fee_per_gas.high;
        let max_priority_fee_per_gas = fees.max_fee_per_gas.high;

        // 5. CREATE NEW SIGNATURE (WRAPPER) ORDER
        let signature_order_data = SignatureOrderData {
            transaction: OrderTransaction::Eip1559 {
                to: format!("{:?}", forwarder_address),
                gas: 200000.into(),
                max_fee_per_gas,
                value: 0.into(),
                max_priority_fee_per_gas,
                data: encoded_data,
                chain_id,
                nonce: None,
            },
            address: gas_pool_address,
            key_id: gas_pool_address_key.key_id,
            maestro_signature: None,
        };

        let sign_order_data_json = serde_json::to_value(&signature_order_data).map_err(|e| {
            OrchestrationError::from(anyhow!(e).context("Unable to serialize signature order data"))
        })?;

        let order_id = Uuid::new_v4();
        let client_id = sponsored_order.data.shared_data.client_id.clone();

        let wrapper_order = OrderStatus {
            order_id,
            order_version: "1".to_string(),
            state: OrderState::Received,
            transaction_hash: None,
            data: GenericOrderData {
                shared_data: SharedOrderData {
                    client_id: client_id.clone(),
                },
                data: sign_order_data_json,
            },
            created_at: Utc::now(),
            order_type: OrderType::Signature,
            last_modified_at: Utc::now(),
            replaced_by: None,
            replaces: Some(request.context.order_id),
            error: None,
            policy: None,
            cancellation_requested: None,
        };

        state
            .orders_repository
            .create_replacement_order(&wrapper_order)
            .await
            .map_err(|e| {
                OrchestrationError::from(anyhow!(e).context("Error creating wrapped order"))
            })?;

        // 6. SEND TO APPROVERS FLOW
        let steps_config = StepFunctionConfig::from(&state.config);
        let steps_function_request = serde_json::to_value(&SendToApproversStateMachineRequest {
            context: StepFunctionContext {
                order_id: wrapper_order.order_id,
            },
            payload: signature_order_data,
        })
        .map_err(|e| {
            OrchestrationError::from(
                anyhow!(e).context("Unable to create state machine json payload"),
            )
        })?;

        invoke_step_function_async_dyn_client(
            client_id,
            steps_function_request,
            state.step_functions_client.as_ref(),
            &steps_config,
            order_id.to_string(),
        )
        .await
        .map_err(|e| {
            OrchestrationError::from(anyhow!(e).context("Error calling approvers state machine"))
        })?;

        Ok(Event {
            payload: TransactionBundlerResponse {
                order_id: wrapper_order.order_id,
            },
            context: request.context,
        })
    }
}

lambda_main!(TransactionBundler);

#[cfg(test)]
mod tests {
    use crate::config::Config;
    use crate::dtos::TransactionBundlerRequest;
    use crate::{Persisted, TransactionBundler};
    use ana_tools::config_loader::ConfigLoader;
    use async_trait::async_trait;
    use chrono::Utc;
    use common::test_tools::http::constants::{
        ADDRESS_FOR_MOCK_REQUESTS, ADDRESS_OF_RECIPIENT_FOR_MOCK_REQUESTS,
        CHAIN_ID_FOR_MOCK_REQUESTS, CLIENT_ID_FOR_MOCK_REQUESTS, KEY_ID_FOR_MOCK_REQUESTS,
    };
    use common::test_tools::mocks::step_client::MockStepsClient;
    use ethers::types::Bytes;
    use ethers::types::{Address, Transaction, U256};
    use mockall::mock;
    use mockall::predicate;
    use mockall::predicate::eq;
    use model::order::{
        GenericOrderData, OrderState, OrderStatus, OrderTransaction, OrderType, SharedOrderData,
        SignatureOrderData, SponsorAddresses,
    };
    use mpc_signature_sm::blockchain::providers::*;
    use mpc_signature_sm::lambda_structure::event::{Event, EventContext};
    use mpc_signature_sm::lambda_structure::lambda_trait::Lambda;
    use repositories::keys::*;
    use repositories::orders::MockOrdersRepository;
    use repositories::orders::OrdersRepository;
    use rstest::*;
    use rusoto_dynamodb::AttributeValue;
    use rusoto_stepfunctions::StartExecutionOutput;
    use serde::Serialize;
    use serde_json::json;
    use serde_json::Value;
    use std::collections::HashMap;
    use std::str::FromStr;

    use model::key::Key;
    use std::sync::Arc;
    use uuid::Uuid;

    #[derive(Serialize)]
    struct KeyDynamoDbResource {
        pub key_id: String,
        pub address: String,
        pub client_id: String,
        pub client_user_id: String,
        pub created_at: String,
        pub order_type: String,
        pub order_version: String,
        pub owning_user_id: String,
        pub public_key: String,
    }

    mock! {
        BlockchainProvider {}
        #[async_trait]
        impl EvmBlockchainProvider for BlockchainProvider {

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
        pub blockchain_provider: MockBlockchainProvider,
        pub orders_repository: MockOrdersRepository,
        pub keys_repository: MockKeysRepository,
        pub step_functions_client: MockStepsClient,
    }

    #[fixture]
    fn fixture() -> TestFixture {
        TestFixture {
            config: ConfigLoader::load_default::<Config>(),
            blockchain_provider: MockBlockchainProvider::new(),
            orders_repository: MockOrdersRepository::new(),
            keys_repository: MockKeysRepository::new(),
            step_functions_client: MockStepsClient::new(),
        }
    }

    impl TestFixture {
        pub fn get_state(self) -> Persisted {
            Persisted {
                config: self.config,
                blockchain_provider: Arc::new(self.blockchain_provider),
                orders_repository: Arc::new(self.orders_repository) as Arc<dyn OrdersRepository>,
                keys_repository: Arc::new(self.keys_repository) as Arc<dyn KeysRepository>,
                step_functions_client: Arc::new(self.step_functions_client),
            }
        }
    }

    fn build_typed_data() -> Value {
        json!(
            {
            "domain": {
                "chainId": "0xaa36a7",
                "name": "test",
                "verifyingContract":"0x0000000000000000000000000000000000000000",
                "version": "1"
            },
            "message": {
                "from": "0x56ca23be3a144dfba70606566cfbb384c5ae96b2" ,
                "to": ADDRESS_OF_RECIPIENT_FOR_MOCK_REQUESTS,
                "value": "0x1",
                "gas": "75000",
                "nonce": "0",
                "deadline": "1807594318",
                "data": "0x00"
            },
            "primaryType": "ForwardRequest",
            "types": {
                "EIP712Domain": [
                    {
                        "name": "name",
                        "type": "string"
                    },
                    {
                        "name": "version",
                        "type": "string"
                    },
                    {
                        "name": "chainId",
                        "type": "string"
                    },
                    {
                        "name": "verifyingContract",
                        "type": "address"
                    }
                ],
                "ForwardRequest": [
                    {
                        "name": "from",
                        "type": "address"
                    },
                    {
                        "name": "to",
                        "type": "address"
                    },
                    {
                        "name": "value",
                        "type": "string"
                    },
                    {
                        "name": "gas",
                        "type": "string"
                    },
                    {
                        "name": "nonce",
                        "type": "string"
                    },
                    {
                        "name": "deadline",
                        "type": "string"
                    },
                    {
                        "name": "data",
                        "type": "bytes"
                    }
                ]
            }
        })
    }

    fn create_sponsored_order_status(order_id: Uuid) -> OrderStatus {
        let signature_order_data = SignatureOrderData {
            transaction: OrderTransaction::Sponsored {
                typed_data: serde_json::from_value(build_typed_data()).unwrap(),
                chain_id: CHAIN_ID_FOR_MOCK_REQUESTS,
                to: Address::from_str(ADDRESS_OF_RECIPIENT_FOR_MOCK_REQUESTS).unwrap(),
                sponsor_addresses: SponsorAddresses {
                    gas_pool_address: Address::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap(),
                    forwarder_address: Address::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap(),
                    forwarder_name: "Forwarder 1".to_owned()
                }
            },
            address: Address::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap(),
            key_id: Uuid::parse_str(KEY_ID_FOR_MOCK_REQUESTS).unwrap(),
            maestro_signature: Some(Bytes::from_str("0x02f86583aa36a78001018255f0943efdd74dd510542ff7d7e4ac1c7039e4901f3ab10100c080a0ed3a3333026ba54f95103ce14d583d6b308e47efdbc1553cb8d47576c3cfe79ea01285de0f3b8366411a62f2c82e6c1f4e92209e9ccca492c5b9f7a6e6e1b51c4c").unwrap()),
        };

        let sign_order_data_json = serde_json::to_value(&signature_order_data).unwrap();

        OrderStatus {
            order_id,
            order_version: "1".to_owned(),
            state: OrderState::Received,
            data: GenericOrderData {
                shared_data: SharedOrderData {
                    client_id: CLIENT_ID_FOR_MOCK_REQUESTS.to_string(),
                },
                data: sign_order_data_json,
            },
            order_type: OrderType::Sponsored, // Sponsored Order
            created_at: Utc::now(),
            last_modified_at: Utc::now(),
            transaction_hash: None,
            replaced_by: None,
            replaces: None,
            error: None,
            policy: None,
            cancellation_requested: None,
        }
    }

    fn get_key_attributes_map() -> HashMap<String, AttributeValue> {
        serde_dynamo::to_item(KeyDynamoDbResource {
            key_id: KEY_ID_FOR_MOCK_REQUESTS.to_owned(),
            address: ADDRESS_FOR_MOCK_REQUESTS.to_owned(),
            client_id: CLIENT_ID_FOR_MOCK_REQUESTS.to_owned(),
            client_user_id: Uuid::default().to_string(),
            created_at: "2023-05-03T16:09:16.997Z".to_owned(),
            order_type: "KEY_CREATION_ORDER".to_owned(),
            order_version: "1".to_owned(),
            owning_user_id: Uuid::default().to_string(),
            public_key: "03762674801475f7a088b26c8cb74d7ccccbd13a7025ed6e38c13b4f261167737c"
                .to_owned(),
        })
        .unwrap()
    }

    fn build_input() -> Event<TransactionBundlerRequest> {
        Event {
            payload: TransactionBundlerRequest {
                maestro_signature: "no".to_owned(),
            },
            context: EventContext {
                order_id: Uuid::new_v4(),
                order_timestamp: Utc::now(),
            },
        }
    }

    #[rstest]
    #[tokio::test]
    async fn bundle_sponsored_transaction_successfully(mut fixture: TestFixture) {
        let request = build_input();
        let order_id = request.context.order_id;

        fixture
            .step_functions_client
            .expect_start_execution()
            .times(1)
            .returning(|_| Ok(StartExecutionOutput::default()));

        fixture
            .blockchain_provider
            .expect_get_evm_endpoint()
            .once()
            .returning(|_, _| Ok("http://127.0.0.1:3000".to_string()));

        fixture
            .blockchain_provider
            .expect_get_fees_from_pending()
            .once()
            .returning(|_| {
                Ok(BlockFeeQuery {
                    base_fee_per_gas: 0.into(),
                    max_priority_fees: vec![0.into()],
                })
            });

        fixture
            .orders_repository
            .expect_get_order_by_id()
            .with(eq(order_id.clone().to_string()))
            .once()
            .returning(move |_| Ok(create_sponsored_order_status(order_id)));

        fixture
            .orders_repository
            .expect_create_replacement_order()
            .once()
            .returning(move |_| Ok(()));

        fixture
            .keys_repository
            .expect_get_key_by_address()
            .with(predicate::eq(
                Address::from_str(ADDRESS_FOR_MOCK_REQUESTS).unwrap(),
            ))
            .once()
            .returning(move |_| {
                let key: Key = serde_dynamo::from_item(get_key_attributes_map()).unwrap();
                Ok(key)
            });

        TransactionBundler::run(request.clone(), &fixture.get_state())
            .await
            .unwrap();
    }
}
