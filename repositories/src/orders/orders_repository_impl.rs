use std::collections::HashMap;

use anyhow::{anyhow, Error};
use async_trait::async_trait;
use chrono::{Duration, Utc};
use common::serializers::h160::h160_to_lowercase_hex_string;
use ethers::prelude::H160;
use rusoto_core::RusotoError;
use rusoto_dynamodb::{
    AttributeValue, Delete, DynamoDb, GetItemInput, Put, PutItemInput, QueryInput,
    TransactWriteItem, TransactWriteItemsInput, Update, UpdateItemError, UpdateItemInput,
};
use serde::de::DeserializeOwned;
use serde_dynamo::rusoto_dynamodb_0_48::to_attribute_value;

use model::cache::DataType;
use model::order::{OrderPK, OrderState, OrderStatus, OrderSummary, OrderTransaction, OrderType};

use crate::orders::input_builder::{
    build_current_states_condition_expression, build_update_order_statement_for_block_num_and_hash,
    compose_attribute_values, compose_update_expression, UpdateOrderStatement,
};
use crate::{
    deserialize::deserialize_from_dynamo,
    orders::{
        KeyChainTypeGSI, OrdersRepository, OrdersRepositoryError, TransactionHashGSI,
        UpdateReplacedBy, UpdateState, KEY_CHAIN_TYPE_INDEX_NAME,
        STATE_LAST_MODIFIED_AT_INDEX_NAME, TRANSACTION_HASH_INDEX_NAME,
    },
};

use super::QueryByStatusAndLastModified;

pub struct OrdersRepositoryImpl<T: DynamoDb + Sync + Send> {
    table_name: String,
    dynamodb_client: T,
}

impl<T: DynamoDb + Sync + Send> OrdersRepositoryImpl<T> {
    pub fn new(table_name: String, dynamodb_client: T) -> Self {
        Self {
            table_name,
            dynamodb_client,
        }
    }

    fn build_order_id_item_input(&self, order_id: String) -> Result<GetItemInput, anyhow::Error> {
        let key = serde_dynamo::to_item(OrderPK { order_id })
            .map_err(|e| anyhow!(e).context("Error building query for orders by order_id"))?;

        Ok(GetItemInput {
            key,
            table_name: self.table_name.clone(),
            ..GetItemInput::default()
        })
    }

    fn build_transaction_hash_query_input(
        &self,
        hash: String,
    ) -> Result<QueryInput, anyhow::Error> {
        let key_condition_expression = "transaction_hash = :hash".to_owned();

        let expression_attribute_values = serde_dynamo::to_item(TransactionHashGSI { hash })
            .map_err(|e| {
                anyhow!(e).context("Error building query for orders by transaction hash")
            })?;

        Ok(QueryInput {
            table_name: self.table_name.clone(),
            index_name: Some(TRANSACTION_HASH_INDEX_NAME.to_owned()),
            key_condition_expression: Some(key_condition_expression),
            expression_attribute_values: Some(expression_attribute_values),
            ..QueryInput::default()
        })
    }

    fn build_update_item_input(
        &self,
        order_id: String,
        new_state: OrderState,
    ) -> Result<UpdateItemInput, anyhow::Error> {
        let update_expression =
            "SET #state = :state, last_modified_at = :last_modified_at".to_owned();

        let expression_attribute_values = serde_dynamo::to_item(UpdateState {
            state: new_state,
            last_modified_at: Utc::now(),
        })
        .map_err(|e| anyhow!(e).context("Error building update order expression"))?;

        let expression_attribute_names =
            HashMap::from([(String::from("#state"), String::from("state"))]);

        let key = serde_dynamo::to_item(OrderPK { order_id })
            .map_err(|e| anyhow!(e).context("Error building update order key"))?;

        Ok(UpdateItemInput {
            key,
            table_name: self.table_name.clone(),
            update_expression: Some(update_expression),
            expression_attribute_values: Some(expression_attribute_values),
            expression_attribute_names: Some(expression_attribute_names),
            ..Default::default()
        })
    }

    // Look up address and chain_id and concatenate them
    // to get a key to where the order might be stored in the cache table.
    async fn get_cache_table_key_by_order_id(
        &self,
        order_id: String,
    ) -> Result<HashMap<String, AttributeValue>, Error> {
        // find address and chain_id for this order, then combine them to form SK for cache table
        let order_status = self
            .get_order_by_id(order_id.clone())
            .await
            .map_err(|e| anyhow!(e).context("Error retrieving order from db"))?;
        tracing::info!(
            order_id = ?order_id, "order_status: {:?}", order_status
        );
        let (address, chain_id) = order_status
            .data
            .extract_address_and_chain_id()
            .map_err(|e| anyhow!(e).context("Failed to find address or chain_id for the order"))?;

        let address_chain_id_string = self
            .build_address_chain_id_key(address, chain_id)
            .map_err(|e| anyhow!(e).context("Error building address_chain_id key"))?;

        let pk = to_attribute_value(DataType::AddressLock).map_err(|e| {
            anyhow!(e).context("Error transforming AddressLock pk to a DynamoDb attribute value")
        })?;

        let address_chain_id_key_as_attr = to_attribute_value(address_chain_id_string)?;
        Ok(HashMap::from([
            ("pk".to_string(), pk),
            ("sk".to_string(), address_chain_id_key_as_attr),
        ]))
    }

    async fn build_unlock_address_transaction_item(
        &self,
        locking_order_id: String,
        cache_table_key: HashMap<String, AttributeValue>,
        cache_table_name: String,
    ) -> Result<Delete, anyhow::Error> {
        let order_id_as_attr = to_attribute_value(locking_order_id.clone())?;
        let expression_attribute_values =
            HashMap::from([(":order_id".to_string(), order_id_as_attr)]);

        Ok(Delete {
            key: cache_table_key,
            table_name: cache_table_name,
            condition_expression: Some(
                "order_id = :order_id OR attribute_not_exists(order_id)".to_string(),
            ),
            expression_attribute_values: Some(expression_attribute_values),
            ..Default::default()
        })
    }

    fn build_address_chain_id_key(
        &self,
        address: H160,
        chain_id: u64,
    ) -> Result<String, anyhow::Error> {
        Ok(format!(
            "ADDRESS#{}#CHAIN_ID#{chain_id}",
            h160_to_lowercase_hex_string(address)
        ))
    }

    fn build_update_order_status_transaction_item(
        &self,
        order_id: String,
        new_state: OrderState,
        update_order_statement: Option<UpdateOrderStatement>,
    ) -> Result<Update, anyhow::Error> {
        let update_expression = compose_update_expression(update_order_statement.clone());

        let expected_current_states = OrderState::get_possible_current_state(new_state);
        let (condition_expression, current_states_attribute_values) =
            build_current_states_condition_expression(expected_current_states);

        let expression_attribute_values = compose_attribute_values(
            new_state,
            current_states_attribute_values,
            update_order_statement.clone(),
        )
        .map_err(|e| anyhow!(e).context("Error parsing attribute values"))?;

        let mut expression_attribute_names =
            HashMap::from([(String::from("#state"), String::from("state"))]);
        if let Some(update_order_data) = update_order_statement {
            expression_attribute_names
                .extend(update_order_data.attribute_names.unwrap_or_default());
        }

        let key = serde_dynamo::to_item(OrderPK {
            order_id: order_id.clone(),
        })
        .map_err(|e| {
            tracing::error!(
                order_id = ?order_id, "Error building PK for this order: {:?}", e
            );
            anyhow!(e).context("Error building order_status update key")
        })?;

        Ok(Update {
            key,
            table_name: self.table_name.clone(),
            update_expression,
            condition_expression: Some(condition_expression),
            expression_attribute_values: Some(expression_attribute_values),
            expression_attribute_names: Some(expression_attribute_names),
            ..Default::default()
        })
    }

    fn build_tx_monitor_update_item_input(
        &self,
        order_id: String,
        new_state: OrderState,
    ) -> Result<UpdateItemInput, anyhow::Error> {
        let update_expression = "SET #state = :state, last_modified_at = :last_modified_at, tx_monitor_last_modified_at = :last_modified_at".to_owned();

        let expression_attribute_values = serde_dynamo::to_item(UpdateState {
            state: new_state,
            last_modified_at: Utc::now(),
        })
        .map_err(|e| anyhow!(e).context("Error building update order expression"))?;

        let expression_attribute_names =
            HashMap::from([(String::from("#state"), String::from("state"))]);

        let key = serde_dynamo::to_item(OrderPK { order_id })
            .map_err(|e| anyhow!(e).context("Error building update order key"))?;

        Ok(UpdateItemInput {
            key,
            table_name: self.table_name.clone(),
            update_expression: Some(update_expression),
            expression_attribute_values: Some(expression_attribute_values),
            expression_attribute_names: Some(expression_attribute_names),
            ..Default::default()
        })
    }

    fn build_status_query_input(
        &self,
        order_state: OrderState,
        last_modified_threshold: i64,
    ) -> Result<QueryInput, anyhow::Error> {
        let state = order_state.to_string();
        let utc_date = Utc::now() - Duration::minutes(last_modified_threshold);
        let last_modified_at = utc_date.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();

        let key_condition_expression =
            "#state = :current_state AND last_modified_at < :last_modified_at".to_owned();

        let expression_attribute_values: HashMap<String, AttributeValue> =
            serde_dynamo::to_item(QueryByStatusAndLastModified {
                state,
                last_modified_at,
            })
            .map_err(|e| anyhow!(e).context("Error building query for orders by state"))?;

        let mut expression_attribute_names = std::collections::HashMap::new();
        expression_attribute_names.insert("#state".to_string(), "state".to_string());

        Ok(QueryInput {
            table_name: self.table_name.clone(),
            index_name: Some(STATE_LAST_MODIFIED_AT_INDEX_NAME.to_owned()),
            key_condition_expression: Some(key_condition_expression),
            expression_attribute_values: Some(expression_attribute_values),
            expression_attribute_names: Some(expression_attribute_names),
            ..QueryInput::default()
        })
    }

    fn build_key_chain_type_state_query_input(
        &self,
        key_id: String,
        chain_id: u64,
        order_type: OrderType,
        state: OrderState,
        limit: Option<i64>,
        exclusive_start_key: Option<HashMap<String, AttributeValue>>,
    ) -> Result<QueryInput, Error> {
        let key_condition_expression =
            "key_chain_type = :key_chain_type AND #state = :state".to_owned();

        let key_chain_type = self
            .build_key_chain_type(key_id, chain_id, order_type)
            .map_err(|e| anyhow!(e).context("Error getting key_chain_type info"))?;

        let expression_attribute_values = serde_dynamo::to_item(KeyChainTypeGSI {
            state: state.to_string(),
            key_chain_type,
        })
        .map_err(|e| anyhow!(e).context("Error building query for orders by status"))?;

        let expression_attribute_names =
            HashMap::from([(String::from("#state"), String::from("state"))]);

        Ok(QueryInput {
            table_name: self.table_name.clone(),
            index_name: Some(KEY_CHAIN_TYPE_INDEX_NAME.to_owned()),
            key_condition_expression: Some(key_condition_expression),
            expression_attribute_values: Some(expression_attribute_values),
            expression_attribute_names: Some(expression_attribute_names),
            limit,
            exclusive_start_key,
            ..QueryInput::default()
        })
    }

    fn build_key_chain_type_from_order(
        &self,
        order_status: &OrderStatus,
    ) -> Result<String, anyhow::Error> {
        let data = order_status.signature_data()?;
        let chain_id = match data.data.transaction {
            OrderTransaction::Legacy { chain_id, .. } => chain_id,
            OrderTransaction::Eip1559 { chain_id, .. } => chain_id,
            OrderTransaction::Sponsored { chain_id, .. } => chain_id,
        };
        self.build_key_chain_type(
            data.data.key_id.to_string(),
            chain_id,
            order_status.order_type,
        )
    }

    fn build_key_chain_type(
        &self,
        key_id: String,
        chain_id: u64,
        order_type: OrderType,
    ) -> Result<String, anyhow::Error> {
        Ok(format!(
            "key_id:{}#chain_id:{}#order_type:{}",
            key_id, chain_id, order_type
        ))
    }

    fn key_chain_type(&self, order_status: &OrderStatus) -> Result<String, anyhow::Error> {
        match order_status.order_type {
            OrderType::KeyCreation => Ok("".to_string()),
            _ => self.build_key_chain_type_from_order(order_status),
        }
    }

    fn build_create_order_item_input(
        &self,
        order_status: &OrderStatus,
    ) -> Result<PutItemInput, anyhow::Error> {
        let mut item: HashMap<String, AttributeValue> = serde_dynamo::to_item(order_status)
            .map_err(|e| {
                OrdersRepositoryError::Unknown(anyhow!(e).context("Error serializing order"))
            })?;

        // Additional fields that are not part of the model but are stored in the same table.
        // TODO we can eventually create a intermediate DynamoDB resource instead of doing this.
        item.insert(
            "execution_arn".to_owned(),
            serde_dynamo::AttributeValue::M(HashMap::new()).into(),
        );
        //

        let key_chain_type = self.key_chain_type(order_status).map_err(|e| {
            OrdersRepositoryError::Unknown(
                anyhow!(e).context("Error getting key_chain_type from order"),
            )
        })?;

        item.insert(
            "key_chain_type".to_owned(),
            serde_dynamo::AttributeValue::S(key_chain_type).into(),
        );

        let input = PutItemInput {
            item,
            table_name: self.table_name.clone(),
            ..PutItemInput::default()
        };

        Ok(input)
    }

    // This private method is expected to be called only for structs following the Order schema.
    // Pagination is implemented to get all the result of the query if there is no limit set.
    // If a limit is provided the method will return when the current amount of items will be
    // greater or equals to the limit.
    // If no limit is provided, the method will do all the necessary queries to retrieve all the
    // items that matches the query.
    async fn get_orders_by_key_chain_type_state_limit<O: DeserializeOwned>(
        &self,
        key_id: String,
        chain_id: u64,
        order_type: OrderType,
        state: OrderState,
        limit: Option<i64>,
    ) -> Result<Vec<O>, OrdersRepositoryError> {
        let mut orders = Vec::new();

        let mut last_key: Option<HashMap<String, AttributeValue>> = None;

        let mut internal_limit = limit;

        loop {
            let input = self.build_key_chain_type_state_query_input(
                key_id.clone(),
                chain_id,
                order_type,
                state,
                internal_limit,
                last_key,
            )?;

            let result = self.dynamodb_client.query(input).await.map_err(|e| {
                OrdersRepositoryError::Unknown(
                    anyhow!(e).context(format!("Error querying order: {}", state.clone())),
                )
            })?;

            let items = result.items;

            if let Some(items) = items {
                for item in items {
                    let order = serde_dynamo::from_item(item).map_err(|e| {
                        OrdersRepositoryError::Unknown(
                            anyhow!(e)
                                .context(format!("Error deserializing order: {}", state.clone())),
                        )
                    })?;

                    orders.push(order);
                }
            }

            if let Some(l) = limit {
                let l = usize::try_from(l).map_err(|e| {
                    OrdersRepositoryError::Unknown(
                        anyhow!(e).context(format!("Error converting limit {}", l)),
                    )
                })?;

                if orders.len() >= l {
                    tracing::info!(
                    key_id = ?key_id,
                    chain_id,
                    order_type = ?order_type,
                    order_state = ?state,
                    "limit fulfilled for key_id {:?} chain_id {}", key_id, chain_id);
                    break;
                } else {
                    internal_limit = Some((l - orders.len()) as i64);
                }
            }

            if let Some(key) = result.last_evaluated_key {
                last_key = Some(key);

                tracing::info!(
                    key_id = ?key_id,
                    chain_id,
                    order_type = ?order_type,
                    order_state = ?state,
                    "last evaluated key found for key_id {:?} chain_id {}", key_id, chain_id);
            } else {
                tracing::info!(
                    key_id = ?key_id,
                    chain_id,
                    order_type = ?order_type,
                    order_state = ?state,
                    "no more items found for key_id {:?} chain_id {}", key_id, chain_id);
                break;
            }
        }

        Ok(orders)
    }
}

#[async_trait]
impl<T: DynamoDb + Sync + Send> OrdersRepository for OrdersRepositoryImpl<T> {
    async fn get_order_by_id(
        &self,
        order_id: String,
    ) -> Result<OrderStatus, OrdersRepositoryError> {
        let input = self.build_order_id_item_input(order_id.clone())?;

        let result = self
            .dynamodb_client
            .get_item(input)
            .await
            .map_err(|e| {
                OrdersRepositoryError::Unknown(
                    anyhow!(e).context(format!("Error querying Order by id: {order_id}")),
                )
            })?
            .item
            .ok_or_else(|| {
                OrdersRepositoryError::OrderNotFound(format!(
                    "Order with id {} not found",
                    order_id.clone()
                ))
            })?;

        deserialize_from_dynamo(result)
    }

    async fn get_orders_by_transaction_hash(
        &self,
        transaction_hash: String,
    ) -> Result<Vec<OrderStatus>, OrdersRepositoryError> {
        let input = self.build_transaction_hash_query_input(transaction_hash.clone())?;

        let items = self
            .dynamodb_client
            .query(input)
            .await
            .map_err(|e| {
                OrdersRepositoryError::Unknown(anyhow!(e).context(format!(
                    "Error querying Order transaction_hash: {}",
                    transaction_hash.clone()
                )))
            })?
            .items
            .ok_or(OrdersRepositoryError::OrderNotFound(format!(
                "Order with hash {} not found",
                transaction_hash.clone()
            )))?;

        let mut orders = Vec::with_capacity(items.len());
        for item in items {
            orders.push(deserialize_from_dynamo::<OrderStatus, OrdersRepositoryError>(item)?);
        }

        Ok(orders)
    }

    async fn get_orders_by_transaction_hashes(
        &self,
        transaction_hashes: Vec<String>,
    ) -> Result<Vec<OrderStatus>, OrdersRepositoryError> {
        let mut orders = Vec::new();
        for transaction_hash in transaction_hashes {
            let items = self
                .get_orders_by_transaction_hash(transaction_hash)
                .await?;
            for item in items {
                orders.push(item);
            }
        }
        Ok(orders)
    }

    async fn get_orders_by_status(
        &self,
        state: OrderState,
        last_modified_threshold: i64,
    ) -> Result<Vec<OrderStatus>, OrdersRepositoryError> {
        let input = self.build_status_query_input(state, last_modified_threshold)?;

        let items = self
            .dynamodb_client
            .query(input)
            .await
            .map_err(|e| {
                eprintln!("Error querying DynamoDB: {:?}", e);
                OrdersRepositoryError::Unknown(anyhow!(e).context(format!(
                    "Error querying Order Status with state: {} and last modified threshold: {}",
                    state.clone(),
                    last_modified_threshold
                )))
            })?
            .items
            .ok_or(OrdersRepositoryError::OrderNotFound(format!(
                "Order with status {} and last modified threshold {} not found",
                state, last_modified_threshold
            )))?;

        let mut orders = Vec::with_capacity(items.len());
        for item in items {
            orders.push(deserialize_from_dynamo::<OrderStatus, OrdersRepositoryError>(item)?);
        }

        Ok(orders)
    }

    async fn update_order_status(
        &self,
        order_id: String,
        new_state: OrderState,
    ) -> Result<(), OrdersRepositoryError> {
        let update_input = self.build_update_item_input(order_id.clone(), new_state)?;

        self.dynamodb_client
            .update_item(update_input)
            .await
            .map_err(|e| {
                OrdersRepositoryError::Unknown(
                    anyhow!(e).context(format!("Error updating order with id: {}", order_id)),
                )
            })?;

        Ok(())
    }

    async fn update_order_state_and_unlock_address(
        &self,
        cache_table_name: String,
        order_id: String,
        new_state: OrderState,
        update_order_statement: Option<UpdateOrderStatement>,
    ) -> Result<(), OrdersRepositoryError> {
        // create UPDATE order_status table statement
        let update_order_status_transaction_item = self
            .build_update_order_status_transaction_item(
                order_id.clone(),
                new_state,
                update_order_statement,
            )?;
        tracing::info!(
            order_id = ?order_id, "update_order_status_transaction_item {:?}",
            update_order_status_transaction_item
        );

        // get address and chain_id for the order
        let cache_table_key = self
            .get_cache_table_key_by_order_id(order_id.clone())
            .await?;

        // create DELETE a row with address#chain_id from cache table statement
        let unlock_address_transaction_item = self
            .build_unlock_address_transaction_item(
                order_id.clone(),
                cache_table_key,
                cache_table_name,
            )
            .await?;
        tracing::info!(
            order_id = ?order_id, "unlock_address_transaction_item {:?}",
            unlock_address_transaction_item
        );

        // create an atomic transaction for DB from UPDATE and DELETE
        let items = TransactWriteItemsInput {
            transact_items: vec![
                TransactWriteItem {
                    update: Some(update_order_status_transaction_item),
                    ..TransactWriteItem::default()
                },
                TransactWriteItem {
                    delete: Some(unlock_address_transaction_item),
                    ..TransactWriteItem::default()
                },
            ],
            ..TransactWriteItemsInput::default()
        };

        // execute
        self.dynamodb_client
            .transact_write_items(items)
            .await
            .map_err(|e| {
                OrdersRepositoryError::Unknown(
                    anyhow!(e).context(format!("Error updating order with id: {}", order_id)),
                )
            })?;

        Ok(())
    }

    async fn update_order_status_and_execution_id_non_terminal_state(
        &self,
        order_id: String,
        new_state: OrderState,
        update_order_statement: Option<UpdateOrderStatement>,
    ) -> Result<(), OrdersRepositoryError> {
        // create UPDATE order_status table statement
        let update_order_status_transaction_item = self
            .build_update_order_status_transaction_item(
                order_id.clone(),
                new_state,
                update_order_statement,
            )?;
        tracing::info!(
            order_id = ?order_id, "update_order_status_transaction_item {:?}",
            update_order_status_transaction_item
        );

        // create an atomic transaction for DB from UPDATE
        let items = TransactWriteItemsInput {
            transact_items: vec![TransactWriteItem {
                update: Some(update_order_status_transaction_item),
                ..TransactWriteItem::default()
            }],
            ..TransactWriteItemsInput::default()
        };

        // execute
        self.dynamodb_client
            .transact_write_items(items)
            .await
            .map_err(|e| {
                OrdersRepositoryError::Unknown(
                    anyhow!(e).context(format!("Error updating order with id: {}", order_id)),
                )
            })?;

        Ok(())
    }

    async fn update_order_state_with_replacement_and_unlock_address(
        &self,
        cache_table_name: String,
        order_id: String,
        original_order_id: String,
        new_state: OrderState,
        update_order_statement: Option<UpdateOrderStatement>,
    ) -> Result<(), OrdersRepositoryError> {
        // create UPDATE order_status table statement
        let update_order_status_transaction_item = self
            .build_update_order_status_transaction_item(
                order_id.clone(),
                new_state,
                update_order_statement.clone(),
            )?;
        tracing::info!(
            order_id = ?order_id, "update_order_status_transaction_item {:?}",
            update_order_status_transaction_item
        );

        // create UPDATE order_status table statement
        let update_original_order_status_transaction_item = self
            .build_update_order_status_transaction_item(
                original_order_id.clone(),
                new_state,
                update_order_statement,
            )?;
        tracing::info!(
            order_id = ?order_id, "update_original__order_status_transaction_item {:?}",
            update_order_status_transaction_item
        );

        // get address and chain_id for the order
        let cache_table_key = self
            .get_cache_table_key_by_order_id(order_id.clone())
            .await?;

        // Create a DELETE  statement to remove a row with address#chain_id from the cache table
        let unlock_address_transaction_item = self
            .build_unlock_address_transaction_item(
                order_id.clone(),
                cache_table_key,
                cache_table_name,
            )
            .await?;
        tracing::info!(
            order_id = ?order_id, "unlock_address_transaction_item {:?}",
            unlock_address_transaction_item
        );

        // create an atomic transaction for DB from UPDATE and DELETE
        let items = TransactWriteItemsInput {
            transact_items: vec![
                TransactWriteItem {
                    update: Some(update_order_status_transaction_item),
                    ..TransactWriteItem::default()
                },
                TransactWriteItem {
                    delete: Some(unlock_address_transaction_item),
                    ..TransactWriteItem::default()
                },
                TransactWriteItem {
                    update: Some(update_original_order_status_transaction_item),
                    ..TransactWriteItem::default()
                },
            ],
            ..TransactWriteItemsInput::default()
        };

        // execute
        self.dynamodb_client
            .transact_write_items(items)
            .await
            .map_err(|e| {
                OrdersRepositoryError::Unknown(
                    anyhow!(e).context(format!("Error updating order with id: {}", order_id)),
                )
            })?;

        Ok(())
    }

    async fn update_order_status_with_replacement_and_execution_id_non_terminal_state(
        &self,
        order_id: String,
        original_order_id: String,
        new_state: OrderState,
        update_order_statement: Option<UpdateOrderStatement>,
    ) -> Result<(), OrdersRepositoryError> {
        // create UPDATE order_status table statement
        let update_order_status_transaction_item = self
            .build_update_order_status_transaction_item(
                order_id.clone(),
                new_state,
                update_order_statement.clone(),
            )?;
        tracing::info!(
            order_id = ?order_id, "update_order_status_transaction_item {:?}",
            update_order_status_transaction_item
        );

        // create UPDATE order_status table statement
        let update_original_order_status_transaction_item = self
            .build_update_order_status_transaction_item(
                original_order_id.clone(),
                new_state,
                update_order_statement,
            )?;
        tracing::info!(
            order_id = ?order_id, "update_original_order_status_transaction_item {:?}",
            update_order_status_transaction_item
        );

        // create an atomic transaction for DB from UPDATE
        let items = TransactWriteItemsInput {
            transact_items: vec![
                TransactWriteItem {
                    update: Some(update_order_status_transaction_item),
                    ..TransactWriteItem::default()
                },
                TransactWriteItem {
                    update: Some(update_original_order_status_transaction_item),
                    ..TransactWriteItem::default()
                },
            ],
            ..TransactWriteItemsInput::default()
        };

        // execute
        self.dynamodb_client
            .transact_write_items(items)
            .await
            .map_err(|e| {
                OrdersRepositoryError::Unknown(
                    anyhow!(e).context(format!("Error updating order with id: {}", order_id)),
                )
            })?;

        Ok(())
    }

    async fn update_order_status_and_tx_monitor_last_update(
        &self,
        order_id: String,
        new_state: OrderState,
    ) -> Result<(), OrdersRepositoryError> {
        let update_input = self.build_tx_monitor_update_item_input(order_id.clone(), new_state)?;

        self.dynamodb_client
            .update_item(update_input)
            .await
            .map_err(|e| {
                OrdersRepositoryError::Unknown(
                    anyhow!(e).context(format!("Error updating order with id: {}", order_id)),
                )
            })?;

        Ok(())
    }

    async fn update_order_status_block(
        &self,
        cache_table_name: String,
        order_id: String,
        new_state: OrderState,
        block_number: u64,
        block_hash: String,
    ) -> Result<(), OrdersRepositoryError> {
        let update_order_statement =
            build_update_order_statement_for_block_num_and_hash(block_number, block_hash)?;

        Ok(self
            .update_order_state_and_unlock_address(
                cache_table_name,
                order_id,
                new_state,
                Some(update_order_statement),
            )
            .await?)
    }

    async fn update_order_and_replacement_with_status_block(
        &self,
        cache_table_name: String,
        mined_order_id: String,
        replaced_order_id: String,
        mined_new_state: OrderState,
        replaced_new_state: OrderState,
        block_number: u64,
        block_hash: String,
        locking_order_id: String,
        replaced_by_order_id: Option<String>,
    ) -> Result<(), OrdersRepositoryError> {
        let update_order_statement =
            build_update_order_statement_for_block_num_and_hash(block_number, block_hash)?;

        // create UPDATE order_status table statement for the mined order
        let update_mined_order_status_transaction_item = self
            .build_update_order_status_transaction_item(
                mined_order_id.clone(),
                mined_new_state,
                Some(update_order_statement),
            )?;
        tracing::info!(
            order_id = ?mined_order_id, "update_mined_order_status_transaction_item {:?}",
            update_mined_order_status_transaction_item
        );

        // create UPDATE order_status table statement for the replaced order
        let replaced_by_data = replaced_by_order_id.map(|r| UpdateOrderStatement {
            assignment_pairs: Some(HashMap::from([(
                "replaced_by".to_owned(),
                ":replaced_by".to_owned(),
            )])),
            attribute_names: None,
            attribute_values: Some(HashMap::from([(
                ":replaced_by".to_owned(),
                serde_dynamo::AttributeValue::S(r).into(),
            )])),
        });
        let update_replaced_order_status_transaction_item = self
            .build_update_order_status_transaction_item(
                replaced_order_id.clone(),
                replaced_new_state,
                replaced_by_data,
            )?;
        tracing::info!(
            order_id = ?replaced_order_id, "update_replaced_order_status_transaction_item {:?}",
            update_replaced_order_status_transaction_item
        );

        // get address and chain_id for the order
        let cache_table_key = self
            .get_cache_table_key_by_order_id(locking_order_id.clone())
            .await?;

        // create DELETE a row with address#chain_id from cache table statement for the mined or replaced order
        let unlock_address_transaction_item = self
            .build_unlock_address_transaction_item(
                locking_order_id.clone(),
                cache_table_key.clone(),
                cache_table_name.clone(),
            )
            .await?;
        tracing::info!(
            locking_order_id = ?locking_order_id, mined_order_id = ?mined_order_id, "unlock_address_transaction_item {:?}",
            unlock_address_transaction_item
        );

        let items = TransactWriteItemsInput {
            transact_items: vec![
                TransactWriteItem {
                    update: Some(update_mined_order_status_transaction_item),
                    ..TransactWriteItem::default()
                },
                TransactWriteItem {
                    update: Some(update_replaced_order_status_transaction_item),
                    ..TransactWriteItem::default()
                },
                TransactWriteItem {
                    delete: Some(unlock_address_transaction_item),
                    ..TransactWriteItem::default()
                },
            ],
            ..TransactWriteItemsInput::default()
        };

        self.dynamodb_client
            .transact_write_items(items)
            .await
            .map_err(|e| {
                tracing::error!(
                    order_id = ?replaced_order_id, "Error updating orders {:?} and {:?}: {:?}",
                    mined_order_id, replaced_order_id, e
                );
                OrdersRepositoryError::Unknown(anyhow!(e).context("Error updating orders"))
            })?;

        Ok(())
    }

    async fn request_cancellation(&self, order_id: String) -> Result<(), OrdersRepositoryError> {
        let update_expression =
            "SET cancellation_requested = :cancelled, last_modified_at = :last_modified_at"
                .to_string();

        let expression_attribute_names = HashMap::from([
            (String::from("#state"), String::from("state")),
            (String::from("#ot"), String::from("order_type")),
        ]);

        let expression_attribute_values: HashMap<String, rusoto_dynamodb::AttributeValue> =
            HashMap::from([
                (
                    ":cancelled".to_owned(),
                    serde_dynamo::AttributeValue::Bool(true).into(),
                ),
                (
                    ":last_modified_at".to_owned(),
                    serde_dynamo::AttributeValue::S(Utc::now().to_string()).into(),
                ),
                (
                    ":state_signed".to_owned(),
                    serde_dynamo::AttributeValue::S(OrderState::Signed.to_string()).into(),
                ),
                (
                    ":state_received".to_owned(),
                    serde_dynamo::AttributeValue::S(OrderState::Received.to_string()).into(),
                ),
                (
                    ":state_approvers_reviewed".to_owned(),
                    serde_dynamo::AttributeValue::S(OrderState::ApproversReviewed.to_string())
                        .into(),
                ),
                (
                    ":state_selected_for_signing".to_owned(),
                    serde_dynamo::AttributeValue::S(OrderState::SelectedForSigning.to_string())
                        .into(),
                ),
                (
                    ":signature_order".to_owned(),
                    serde_dynamo::AttributeValue::S(OrderType::Signature.to_string()).into(),
                ),
            ]);

        // The update item will only succeed if the order is in the one of the states listed in the
        // expression and also the order type is `SIGNATURE_ORDER`
        let condition_expression =
            "#state IN (:state_signed, :state_received, :state_approvers_reviewed, :state_selected_for_signing) AND #ot = :signature_order".to_owned();

        let key = serde_dynamo::to_item(OrderPK {
            order_id: order_id.clone(),
        })
        .map_err(|e| {
            OrdersRepositoryError::Unknown(
                anyhow!(e).context("Error building cancellation order PK"),
            )
        })?;

        let update_input = UpdateItemInput {
            key,
            table_name: self.table_name.clone(),
            update_expression: Some(update_expression),
            condition_expression: Some(condition_expression),
            expression_attribute_names: Some(expression_attribute_names),
            expression_attribute_values: Some(expression_attribute_values),
            ..Default::default()
        };

        self.dynamodb_client
            .update_item(update_input)
            .await
            .map_err(|e| match e {
                RusotoError::Service(UpdateItemError::ConditionalCheckFailed(_)) => {
                    OrdersRepositoryError::ConditionalCheckFailed(
                        "the order is in an uncancellable state".to_owned(),
                    )
                }
                _ => OrdersRepositoryError::Unknown(
                    anyhow!(e).context(format!("Error cancelling order with id: {}", order_id)),
                ),
            })?;

        Ok(())
    }

    async fn create_replacement_order(
        &self,
        new_order: &OrderStatus,
    ) -> Result<(), OrdersRepositoryError> {
        let original_order_id =
            new_order
                .replaces
                .ok_or(OrdersRepositoryError::Unknown(anyhow!(
                    "Missing order replaces"
                )))?;

        let new_order_id = new_order.order_id.to_string();

        let create_order_item_input = self.build_create_order_item_input(new_order)?;

        let update_key = serde_dynamo::to_item(OrderPK {
            order_id: original_order_id.to_string(),
        })
        .map_err(|e| {
            OrdersRepositoryError::Unknown(anyhow!(e).context("Error serializing order key"))
        })?;

        let expression_attribute_values = serde_dynamo::to_item(UpdateReplacedBy {
            replaced_by: new_order_id,
            last_modified_at: Utc::now(),
        })
        .map_err(|e| {
            OrdersRepositoryError::Unknown(
                anyhow!(e).context("Error serializing order replaced by"),
            )
        })?;

        let items = TransactWriteItemsInput {
            transact_items: vec![
                TransactWriteItem {
                    put: Some(Put {
                        item: create_order_item_input.item,
                        table_name: create_order_item_input.table_name,
                        ..Put::default()
                    }),
                    ..TransactWriteItem::default()
                },
                TransactWriteItem {
                    update: Some(Update {
                        expression_attribute_values: Some(expression_attribute_values),
                        key: update_key,
                        table_name: self.table_name.clone(),
                        update_expression:
                            "SET replaced_by = :replaced_by, last_modified_at = :last_modified_at"
                                .to_string(),
                        ..Update::default()
                    }),
                    ..TransactWriteItem::default()
                },
            ],
            ..TransactWriteItemsInput::default()
        };

        self.dynamodb_client
            .transact_write_items(items)
            .await
            .map_err(|e| {
                OrdersRepositoryError::Unknown(
                    anyhow!(e).context("Error creating a replacement order"),
                )
            })?;

        Ok(())
    }

    async fn create_order(&self, order: &OrderStatus) -> Result<(), OrdersRepositoryError> {
        let new_order = self.build_create_order_item_input(order)?;

        self.dynamodb_client
            .put_item(new_order)
            .await
            .map_err(|e| {
                OrdersRepositoryError::Unknown(anyhow!(e).context("Error storing the order"))
            })?;

        Ok(())
    }

    async fn get_orders_by_key_chain_type_state(
        &self,
        key_id: String,
        chain_id: u64,
        order_type: OrderType,
        state: OrderState,
        limit: Option<i64>,
    ) -> Result<Vec<OrderStatus>, OrdersRepositoryError> {
        self.get_orders_by_key_chain_type_state_limit(key_id, chain_id, order_type, state, limit)
            .await
    }

    async fn get_orders_summary_by_key_chain_type_state(
        &self,
        key_id: String,
        chain_id: u64,
        order_type: OrderType,
        state: OrderState,
        limit: Option<i64>,
    ) -> Result<Vec<OrderSummary>, OrdersRepositoryError> {
        self.get_orders_by_key_chain_type_state_limit(key_id, chain_id, order_type, state, limit)
            .await
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::str::FromStr;

    use ethers::types::{H160, H256};
    use mockall::predicate::eq;
    use rstest::{fixture, rstest};
    use rusoto_core::RusotoError;
    use rusoto_dynamodb::{
        AttributeValue, GetItemError, GetItemInput, GetItemOutput, PutItemOutput, QueryError,
        QueryOutput, TransactWriteItemsError, TransactWriteItemsOutput, UpdateItemError,
        UpdateItemOutput,
    };
    use uuid::Uuid;

    use common::test_tools::http::constants::{
        CHAIN_ID_FOR_MOCK_REQUESTS, KEY_ID_FOR_MOCK_REQUESTS, ORDER_ID_FOR_MOCK_REQUESTS,
    };
    use common::test_tools::mocks::dynamodb_client::MockDbClient;
    use model::order::helpers::build_signature_order;
    use model::order::{OrderPK, OrderState, OrderStatus, OrderType};

    use crate::orders::orders_repository_impl::OrdersRepositoryImpl;
    use crate::orders::{OrdersRepository, OrdersRepositoryError};

    struct TestFixture {
        pub dynamodb_client: MockDbClient,
        pub cache_table_name: String,
        pub order_status_table_name: String,
    }

    #[fixture]
    fn fixture() -> TestFixture {
        TestFixture {
            dynamodb_client: MockDbClient::new(),
            cache_table_name: "cache".to_owned(),
            order_status_table_name: "order_status".to_owned(),
        }
    }

    #[rstest]
    #[tokio::test]
    async fn get_order_by_id_db_error(mut fixture: TestFixture) {
        fixture
            .dynamodb_client
            .expect_get_item()
            .with(eq(GetItemInput {
                key: serde_dynamo::to_item(OrderPK {
                    order_id: ORDER_ID_FOR_MOCK_REQUESTS.to_owned(),
                })
                .unwrap(),
                table_name: fixture.order_status_table_name.clone(),
                ..GetItemInput::default()
            }))
            .once()
            .returning(|_| {
                Err(RusotoError::Service(GetItemError::InternalServerError(
                    "timeout!".to_owned(),
                )))
            });

        let repo = OrdersRepositoryImpl::new(
            fixture.order_status_table_name.clone(),
            fixture.dynamodb_client,
        );
        let error = repo
            .get_order_by_id(ORDER_ID_FOR_MOCK_REQUESTS.to_owned())
            .await
            .unwrap_err();
        assert!(matches!(error, OrdersRepositoryError::Unknown(_)));
        assert!(error.to_string().contains("timeout!"));
    }

    #[rstest]
    #[tokio::test]
    async fn get_order_by_id_not_found(mut fixture: TestFixture) {
        fixture
            .dynamodb_client
            .expect_get_item()
            .with(eq(GetItemInput {
                key: serde_dynamo::to_item(OrderPK {
                    order_id: ORDER_ID_FOR_MOCK_REQUESTS.to_owned(),
                })
                .unwrap(),
                table_name: fixture.order_status_table_name.clone(),
                ..GetItemInput::default()
            }))
            .once()
            .returning(|_| Ok(GetItemOutput::default()));

        let repo = OrdersRepositoryImpl::new(
            fixture.order_status_table_name.clone(),
            fixture.dynamodb_client,
        );
        let error = repo
            .get_order_by_id(ORDER_ID_FOR_MOCK_REQUESTS.to_owned())
            .await
            .unwrap_err();
        assert!(matches!(error, OrdersRepositoryError::OrderNotFound(_)));
        assert!(error.to_string().contains(ORDER_ID_FOR_MOCK_REQUESTS));
    }

    #[rstest]
    #[tokio::test]
    async fn get_order_by_id_error_deserializing(mut fixture: TestFixture) {
        fixture
            .dynamodb_client
            .expect_get_item()
            .with(eq(GetItemInput {
                key: serde_dynamo::to_item(OrderPK {
                    order_id: ORDER_ID_FOR_MOCK_REQUESTS.to_owned(),
                })
                .unwrap(),
                table_name: fixture.order_status_table_name.clone(),
                ..GetItemInput::default()
            }))
            .once()
            .returning(|_| {
                Ok(GetItemOutput {
                    item: Some(HashMap::default()),
                    ..GetItemOutput::default()
                })
            });

        let repo = OrdersRepositoryImpl::new(
            fixture.order_status_table_name.clone(),
            fixture.dynamodb_client,
        );
        let error = repo
            .get_order_by_id(ORDER_ID_FOR_MOCK_REQUESTS.to_owned())
            .await
            .unwrap_err();
        assert!(matches!(error, OrdersRepositoryError::Unknown(_)));
        assert!(error.to_string().contains("Error deserializing record"));
    }

    #[rstest]
    #[tokio::test]
    async fn get_order_by_id(mut fixture: TestFixture) {
        let expected_order = build_signature_order(Uuid::new_v4(), OrderState::Completed, None);
        let item = Some(serde_dynamo::to_item(&expected_order).unwrap());
        fixture
            .dynamodb_client
            .expect_get_item()
            .with(eq(GetItemInput {
                key: serde_dynamo::to_item(OrderPK {
                    order_id: ORDER_ID_FOR_MOCK_REQUESTS.to_owned(),
                })
                .unwrap(),
                table_name: fixture.order_status_table_name.clone(),
                ..GetItemInput::default()
            }))
            .once()
            .returning(move |_| {
                Ok(GetItemOutput {
                    item: item.clone(),
                    ..GetItemOutput::default()
                })
            });

        let repo = OrdersRepositoryImpl::new(
            fixture.order_status_table_name.clone(),
            fixture.dynamodb_client,
        );
        let result = repo
            .get_order_by_id(ORDER_ID_FOR_MOCK_REQUESTS.to_owned())
            .await
            .unwrap();
        assert_eq!(expected_order.order_id, result.order_id);
        assert_eq!(expected_order.order_version, result.order_version);
        assert_eq!(expected_order.order_version, result.order_version);
        assert_eq!(expected_order.state, result.state);
        assert_eq!(expected_order.transaction_hash, result.transaction_hash);
        assert_eq!(expected_order.data, result.data);
        assert_eq!(expected_order.created_at, result.created_at);
        assert_eq!(expected_order.order_type, result.order_type);
        assert_eq!(expected_order.last_modified_at, result.last_modified_at);
    }

    #[rstest]
    #[tokio::test]
    async fn update_order_status_db_error(mut fixture: TestFixture) {
        fixture
            .dynamodb_client
            .expect_update_item()
            .once()
            .returning(move |_| {
                Err(RusotoError::Service(UpdateItemError::InternalServerError(
                    "timeout!".to_owned(),
                )))
            });

        let repo = OrdersRepositoryImpl::new(
            fixture.order_status_table_name.clone(),
            fixture.dynamodb_client,
        );
        let error = repo
            .update_order_status(ORDER_ID_FOR_MOCK_REQUESTS.to_owned(), OrderState::Completed)
            .await
            .unwrap_err();
        assert!(matches!(error, OrdersRepositoryError::Unknown(_)));
        assert!(error.to_string().contains("timeout!"));
    }

    #[rstest]
    #[tokio::test]
    async fn update_order_status_block_db_error(mut fixture: TestFixture) {
        let expected_order = build_signature_order(Uuid::new_v4(), OrderState::Completed, None);
        let item = Some(serde_dynamo::to_item(&expected_order).unwrap());
        fixture
            .dynamodb_client
            .expect_get_item()
            .with(eq(GetItemInput {
                key: serde_dynamo::to_item(OrderPK {
                    order_id: ORDER_ID_FOR_MOCK_REQUESTS.to_owned(),
                })
                .unwrap(),
                table_name: fixture.order_status_table_name.clone(),
                ..GetItemInput::default()
            }))
            .once()
            .returning(move |_| {
                Ok(GetItemOutput {
                    item: item.clone(),
                    ..GetItemOutput::default()
                })
            });
        fixture
            .dynamodb_client
            .expect_transact_write_items()
            .once()
            .returning(move |_| {
                Err(RusotoError::Service(
                    TransactWriteItemsError::InternalServerError("timeout!".to_owned()),
                ))
            });

        let repo = OrdersRepositoryImpl::new(
            fixture.order_status_table_name.clone(),
            fixture.dynamodb_client,
        );
        let error = repo
            .update_order_status_block(
                fixture.cache_table_name.clone(),
                ORDER_ID_FOR_MOCK_REQUESTS.to_owned(),
                OrderState::Completed,
                1,
                "0x0".to_string(),
            )
            .await
            .unwrap_err();
        assert!(matches!(error, OrdersRepositoryError::Unknown(_)));
        assert!(error.to_string().contains("timeout!"));
    }

    #[rstest]
    #[tokio::test]
    async fn request_cancellation_succeed(mut fixture: TestFixture) {
        fixture
            .dynamodb_client
            .expect_update_item()
            .once()
            .returning(move |_| Ok(UpdateItemOutput::default()));

        let repo = OrdersRepositoryImpl::new(
            fixture.order_status_table_name.clone(),
            fixture.dynamodb_client,
        );
        repo.request_cancellation(ORDER_ID_FOR_MOCK_REQUESTS.to_owned())
            .await
            .expect("should succeed");
    }

    #[rstest]
    #[tokio::test]
    async fn update_order_status(mut fixture: TestFixture) {
        fixture
            .dynamodb_client
            .expect_update_item()
            .once()
            .returning(move |_| Ok(UpdateItemOutput::default()));

        let repo = OrdersRepositoryImpl::new(
            fixture.order_status_table_name.clone(),
            fixture.dynamodb_client,
        );
        repo.update_order_status(ORDER_ID_FOR_MOCK_REQUESTS.to_owned(), OrderState::Completed)
            .await
            .expect("should succeed");
    }

    #[rstest]
    #[tokio::test]
    async fn request_cancellation_failure(mut fixture: TestFixture) {
        fixture
            .dynamodb_client
            .expect_update_item()
            .once()
            .returning(move |_| {
                Err(RusotoError::Service(
                    UpdateItemError::ConditionalCheckFailed(
                        "the order is in an uncancellable state!".to_owned(),
                    ),
                ))
            });

        let repo = OrdersRepositoryImpl::new(
            fixture.order_status_table_name.clone(),
            fixture.dynamodb_client,
        );
        let error = repo
            .request_cancellation(ORDER_ID_FOR_MOCK_REQUESTS.to_owned())
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            OrdersRepositoryError::ConditionalCheckFailed(_)
        ));
        assert!(error
            .to_string()
            .contains("the order is in an uncancellable state"));
    }

    #[rstest]
    #[tokio::test]
    async fn create_replacement_order_status_missing_replaces_error(fixture: TestFixture) {
        let order = build_signature_order(Uuid::new_v4(), OrderState::Submitted, None);

        let repo = OrdersRepositoryImpl::new(
            fixture.order_status_table_name.clone(),
            fixture.dynamodb_client,
        );
        let error = repo.create_replacement_order(&order).await.unwrap_err();
        assert!(matches!(error, OrdersRepositoryError::Unknown(_)));
        assert!(error.to_string().contains("Missing order replaces"));
    }

    #[rstest]
    #[tokio::test]
    async fn create_replacement_order_status_db_error(mut fixture: TestFixture) {
        fixture
            .dynamodb_client
            .expect_transact_write_items()
            .once()
            .returning(move |_| {
                Err(RusotoError::Service(
                    TransactWriteItemsError::InternalServerError("timeout!".to_owned()),
                ))
            });

        let mut order = build_signature_order(Uuid::new_v4(), OrderState::Submitted, None);
        order.replaces = Some(Uuid::new_v4());

        let repo = OrdersRepositoryImpl::new(
            fixture.order_status_table_name.clone(),
            fixture.dynamodb_client,
        );
        let error = repo.create_replacement_order(&order).await.unwrap_err();
        assert!(matches!(error, OrdersRepositoryError::Unknown(_)));
        assert!(error.to_string().contains("timeout!"));
    }

    #[rstest]
    #[tokio::test]
    async fn create_replacement_order_status(mut fixture: TestFixture) {
        fixture
            .dynamodb_client
            .expect_transact_write_items()
            .once()
            .returning(move |_| Ok(TransactWriteItemsOutput::default()));

        let mut order = build_signature_order(Uuid::new_v4(), OrderState::Submitted, None);
        order.replaces = Some(Uuid::new_v4());

        let repo = OrdersRepositoryImpl::new(
            fixture.order_status_table_name.clone(),
            fixture.dynamodb_client,
        );
        repo.create_replacement_order(&order)
            .await
            .expect("should succeed");
    }

    #[rstest]
    #[tokio::test]
    async fn get_orders_by_transaction_hash_db_error(mut fixture: TestFixture) {
        fixture
            .dynamodb_client
            .expect_query()
            .once()
            .returning(|_| {
                Err(RusotoError::Service(QueryError::InternalServerError(
                    "timeout!".to_owned(),
                )))
            });

        let repo = OrdersRepositoryImpl::new(
            fixture.order_status_table_name.clone(),
            fixture.dynamodb_client,
        );
        let error = repo
            .get_orders_by_transaction_hash(H160::random().to_string())
            .await
            .unwrap_err();
        assert!(matches!(error, OrdersRepositoryError::Unknown(_)));
        assert!(error.to_string().contains("timeout!"));
    }

    #[rstest]
    #[tokio::test]
    async fn get_orders_by_transaction_hash_not_found(mut fixture: TestFixture) {
        fixture
            .dynamodb_client
            .expect_query()
            .once()
            .returning(|_| Ok(QueryOutput::default()));

        let repo = OrdersRepositoryImpl::new(
            fixture.order_status_table_name.clone(),
            fixture.dynamodb_client,
        );
        let hash = H160::random().to_string();
        let error = repo
            .get_orders_by_transaction_hash(hash.clone())
            .await
            .unwrap_err();
        assert!(matches!(error, OrdersRepositoryError::OrderNotFound(_)));
        assert!(error.to_string().contains(&hash));
    }

    #[rstest]
    #[tokio::test]
    async fn get_orders_by_transaction_hash(mut fixture: TestFixture) {
        let first_order = build_signature_order(Uuid::new_v4(), OrderState::Completed, None);
        let mut second_order = build_signature_order(Uuid::new_v4(), OrderState::Completed, None);
        second_order.data.data = serde_json::Value::from_str("{}").unwrap();
        let first_item: HashMap<String, AttributeValue> =
            serde_dynamo::to_item(first_order.clone()).unwrap();
        let second_item: HashMap<String, AttributeValue> =
            serde_dynamo::to_item(second_order.clone()).unwrap();
        fixture
            .dynamodb_client
            .expect_query()
            .once()
            .returning(move |_| {
                Ok(QueryOutput {
                    items: Some(vec![first_item.clone(), second_item.clone()]),
                    ..QueryOutput::default()
                })
            });

        let repo = OrdersRepositoryImpl::new(
            fixture.order_status_table_name.clone(),
            fixture.dynamodb_client,
        );
        let result = repo
            .get_orders_by_transaction_hash(ORDER_ID_FOR_MOCK_REQUESTS.to_owned())
            .await
            .unwrap();

        assert_eq!(first_order.order_id, result[0].order_id);
        assert_eq!(first_order.order_version, result[0].order_version);
        assert_eq!(first_order.order_version, result[0].order_version);
        assert_eq!(first_order.state, result[0].state);
        assert_eq!(first_order.transaction_hash, result[0].transaction_hash);
        assert_eq!(first_order.data, result[0].data);
        assert_eq!(first_order.created_at, result[0].created_at);
        assert_eq!(first_order.order_type, result[0].order_type);
        assert_eq!(first_order.last_modified_at, result[0].last_modified_at);

        assert_eq!(second_order.order_id, result[1].order_id);
        assert_eq!(second_order.order_version, result[1].order_version);
        assert_eq!(second_order.order_version, result[1].order_version);
        assert_eq!(second_order.state, result[1].state);
        assert_eq!(second_order.transaction_hash, result[1].transaction_hash);
        assert_eq!(second_order.data, result[1].data);
        assert_eq!(second_order.created_at, result[1].created_at);
        assert_eq!(second_order.order_type, result[1].order_type);
        assert_eq!(second_order.last_modified_at, result[1].last_modified_at);
    }

    #[rstest]
    #[tokio::test]
    async fn get_orders_by_transaction_hashes(mut fixture: TestFixture) {
        let mut orders: Vec<OrderStatus> = vec![];
        let mut items: Vec<HashMap<String, AttributeValue>> = vec![];
        let mut transaction_hashes: Vec<String> = vec![];
        for _ in 0..10 {
            let transaction_hash = H256::random().to_string();
            transaction_hashes.push(transaction_hash.clone());
            let order = build_signature_order(Uuid::new_v4(), OrderState::Submitted, None);
            orders.push(order.clone());
            let item: HashMap<String, AttributeValue> =
                serde_dynamo::to_item(order.clone()).unwrap();
            items.push(item);
        }

        fixture
            .dynamodb_client
            .expect_query()
            .times(10)
            .returning(move |_| {
                Ok(QueryOutput {
                    items: Some(items.clone()),
                    ..QueryOutput::default()
                })
            });

        let repo = OrdersRepositoryImpl::new(
            fixture.order_status_table_name.clone(),
            fixture.dynamodb_client,
        );
        let result = repo
            .get_orders_by_transaction_hashes(transaction_hashes.to_owned())
            .await
            .unwrap();

        for i in 0..10 {
            assert_eq!(orders[i].order_id, result[i].order_id);
            assert_eq!(orders[i].order_version, result[i].order_version);
            assert_eq!(orders[i].order_version, result[i].order_version);
            assert_eq!(orders[i].state, result[i].state);
            assert_eq!(orders[i].transaction_hash, result[i].transaction_hash);
            assert_eq!(orders[i].data, result[i].data);
            assert_eq!(orders[i].created_at, result[i].created_at);
            assert_eq!(orders[i].order_type, result[i].order_type);
            assert_eq!(orders[i].last_modified_at, result[i].last_modified_at);
        }
    }

    #[rstest]
    #[tokio::test]
    async fn get_orders_by_status(mut fixture: TestFixture) {
        let mut orders: Vec<OrderStatus> = vec![];
        let mut items: Vec<HashMap<String, AttributeValue>> = vec![];
        for _ in 0..10 {
            let order = build_signature_order(Uuid::new_v4(), OrderState::Submitted, None);
            orders.push(order.clone());
            let item: HashMap<String, AttributeValue> =
                serde_dynamo::to_item(order.clone()).unwrap();
            items.push(item);
        }

        fixture
            .dynamodb_client
            .expect_query()
            .once()
            .returning(move |_| {
                Ok(QueryOutput {
                    items: Some(items.clone()),
                    ..QueryOutput::default()
                })
            });

        let repo = OrdersRepositoryImpl::new(
            fixture.order_status_table_name.clone(),
            fixture.dynamodb_client,
        );
        let result = repo
            .get_orders_by_transaction_hash(ORDER_ID_FOR_MOCK_REQUESTS.to_owned())
            .await
            .unwrap();

        for i in 0..10 {
            assert_eq!(orders[i].order_id, result[i].order_id);
            assert_eq!(orders[i].order_version, result[i].order_version);
            assert_eq!(orders[i].order_version, result[i].order_version);
            assert_eq!(orders[i].state, result[i].state);
            assert_eq!(orders[i].transaction_hash, result[i].transaction_hash);
            assert_eq!(orders[i].data, result[i].data);
            assert_eq!(orders[i].created_at, result[i].created_at);
            assert_eq!(orders[i].order_type, result[i].order_type);
            assert_eq!(orders[i].last_modified_at, result[i].last_modified_at);
        }
    }

    #[rstest]
    #[tokio::test]
    async fn create_order(mut fixture: TestFixture) {
        let order = build_signature_order(Uuid::new_v4(), OrderState::Submitted, None);

        fixture
            .dynamodb_client
            .expect_put_item()
            .once()
            .returning(move |_| Ok(PutItemOutput::default()));

        let repo = OrdersRepositoryImpl::new(
            fixture.order_status_table_name.clone(),
            fixture.dynamodb_client,
        );
        assert!(repo.create_order(&order).await.is_ok());
    }

    #[rstest]
    #[tokio::test]
    async fn get_orders_by_key_chain_type_state_db_error(mut fixture: TestFixture) {
        fixture
            .dynamodb_client
            .expect_query()
            .once()
            .returning(|_| {
                Err(RusotoError::Service(QueryError::InternalServerError(
                    "timeout!".to_owned(),
                )))
            });

        let repo = OrdersRepositoryImpl::new(
            fixture.order_status_table_name.clone(),
            fixture.dynamodb_client,
        );
        let error = repo
            .get_orders_by_key_chain_type_state(
                KEY_ID_FOR_MOCK_REQUESTS.to_string(),
                CHAIN_ID_FOR_MOCK_REQUESTS,
                OrderType::Signature,
                OrderState::Submitted,
                None,
            )
            .await
            .unwrap_err();
        assert!(matches!(error, OrdersRepositoryError::Unknown(_)));
        assert!(error.to_string().contains("timeout!"));
    }

    #[rstest]
    #[tokio::test]
    async fn get_orders_by_key_chain_type_state_not_found(mut fixture: TestFixture) {
        fixture
            .dynamodb_client
            .expect_query()
            .once()
            .returning(move |_| {
                Ok(QueryOutput {
                    items: None,
                    ..QueryOutput::default()
                })
            });

        let repo = OrdersRepositoryImpl::new(
            fixture.order_status_table_name.clone(),
            fixture.dynamodb_client,
        );
        let result = repo
            .get_orders_by_key_chain_type_state(
                KEY_ID_FOR_MOCK_REQUESTS.to_string(),
                CHAIN_ID_FOR_MOCK_REQUESTS,
                OrderType::Signature,
                OrderState::Submitted,
                None,
            )
            .await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[rstest]
    #[tokio::test]
    async fn get_orders_by_key_chain_type_state_ok(mut fixture: TestFixture) {
        let first_order = build_signature_order(Uuid::new_v4(), OrderState::Completed, None);
        let mut second_order = build_signature_order(Uuid::new_v4(), OrderState::Completed, None);
        second_order.data.data = serde_json::Value::from_str("{}").unwrap();
        let first_item: HashMap<String, AttributeValue> =
            serde_dynamo::to_item(first_order.clone()).unwrap();
        let second_item: HashMap<String, AttributeValue> =
            serde_dynamo::to_item(second_order.clone()).unwrap();

        fixture
            .dynamodb_client
            .expect_query()
            .once()
            .returning(move |_| {
                Ok(QueryOutput {
                    items: Some(vec![first_item.clone(), second_item.clone()]),
                    ..QueryOutput::default()
                })
            });

        let repo = OrdersRepositoryImpl::new(
            fixture.order_status_table_name.clone(),
            fixture.dynamodb_client,
        );
        let result = repo
            .get_orders_by_key_chain_type_state(
                KEY_ID_FOR_MOCK_REQUESTS.to_string(),
                CHAIN_ID_FOR_MOCK_REQUESTS,
                OrderType::Signature,
                OrderState::Submitted,
                None,
            )
            .await
            .unwrap();

        assert_eq!(first_order.order_id, result[0].order_id);
        assert_eq!(first_order.order_version, result[0].order_version);
        assert_eq!(first_order.order_version, result[0].order_version);
        assert_eq!(first_order.state, result[0].state);
        assert_eq!(first_order.transaction_hash, result[0].transaction_hash);
        assert_eq!(first_order.data, result[0].data);
        assert_eq!(first_order.created_at, result[0].created_at);
        assert_eq!(first_order.order_type, result[0].order_type);
        assert_eq!(first_order.last_modified_at, result[0].last_modified_at);

        assert_eq!(second_order.order_id, result[1].order_id);
        assert_eq!(second_order.order_version, result[1].order_version);
        assert_eq!(second_order.order_version, result[1].order_version);
        assert_eq!(second_order.state, result[1].state);
        assert_eq!(second_order.transaction_hash, result[1].transaction_hash);
        assert_eq!(second_order.data, result[1].data);
        assert_eq!(second_order.created_at, result[1].created_at);
        assert_eq!(second_order.order_type, result[1].order_type);
        assert_eq!(second_order.last_modified_at, result[1].last_modified_at);
    }

    #[rstest]
    #[tokio::test]
    async fn get_orders_by_key_chain_type_state_with_pagination_ok(mut fixture: TestFixture) {
        let first_order = build_signature_order(Uuid::new_v4(), OrderState::Completed, None);
        let mut second_order = build_signature_order(Uuid::new_v4(), OrderState::Completed, None);
        second_order.data.data = serde_json::Value::from_str("{}").unwrap();
        let first_item: HashMap<String, AttributeValue> =
            serde_dynamo::to_item(first_order.clone()).unwrap();
        let second_item: HashMap<String, AttributeValue> =
            serde_dynamo::to_item(second_order.clone()).unwrap();

        fixture
            .dynamodb_client
            .expect_query()
            .once()
            .returning(move |_| {
                Ok(QueryOutput {
                    items: Some(vec![first_item.clone()]),
                    last_evaluated_key: Some(HashMap::new()),
                    ..QueryOutput::default()
                })
            });

        fixture
            .dynamodb_client
            .expect_query()
            .once()
            .returning(move |_| {
                Ok(QueryOutput {
                    items: Some(vec![second_item.clone()]),
                    ..QueryOutput::default()
                })
            });

        let repo = OrdersRepositoryImpl::new(
            fixture.order_status_table_name.clone(),
            fixture.dynamodb_client,
        );
        let result = repo
            .get_orders_by_key_chain_type_state(
                KEY_ID_FOR_MOCK_REQUESTS.to_string(),
                CHAIN_ID_FOR_MOCK_REQUESTS,
                OrderType::Signature,
                OrderState::Submitted,
                None,
            )
            .await
            .unwrap();

        assert_eq!(first_order.order_id, result[0].order_id);
        assert_eq!(first_order.order_version, result[0].order_version);
        assert_eq!(first_order.order_version, result[0].order_version);
        assert_eq!(first_order.state, result[0].state);
        assert_eq!(first_order.transaction_hash, result[0].transaction_hash);
        assert_eq!(first_order.data, result[0].data);
        assert_eq!(first_order.created_at, result[0].created_at);
        assert_eq!(first_order.order_type, result[0].order_type);
        assert_eq!(first_order.last_modified_at, result[0].last_modified_at);

        assert_eq!(second_order.order_id, result[1].order_id);
        assert_eq!(second_order.order_version, result[1].order_version);
        assert_eq!(second_order.order_version, result[1].order_version);
        assert_eq!(second_order.state, result[1].state);
        assert_eq!(second_order.transaction_hash, result[1].transaction_hash);
        assert_eq!(second_order.data, result[1].data);
        assert_eq!(second_order.created_at, result[1].created_at);
        assert_eq!(second_order.order_type, result[1].order_type);
        assert_eq!(second_order.last_modified_at, result[1].last_modified_at);
    }
}
