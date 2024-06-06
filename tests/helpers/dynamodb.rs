use std::collections::HashMap;

use model::order::OrderStatus;
use rusoto_dynamodb::{
    AttributeValue, CreateTableInput, DeleteTableInput, DynamoDb, DynamoDbClient, GetItemInput,
    PutItemInput, QueryInput,
};

use serde::{de::DeserializeOwned, Serialize};

pub async fn get_item_from_db<K: Serialize, R: DeserializeOwned>(
    dynamodb_client: &DynamoDbClient,
    table_name: &str,
    key: K,
) -> Option<R> {
    let key = serde_dynamo::to_item(key).expect("Unable to serialize DynamoDb key");
    let result = dynamodb_client
        .get_item(GetItemInput {
            key,
            table_name: table_name.to_string(),
            ..Default::default()
        })
        .await
        .unwrap_or_else(|e| panic!("Unable to get row from DynamoDb.{e:?}",));

    result.item.map(|r| {
        serde_dynamo::from_item::<_, R>(r).expect("Unable to deserialize row from DynamoDb")
    })
}

pub async fn query_from_db<K: Serialize>(
    dynamodb_client: &DynamoDbClient,
    table_name: &str,
    key_condition_expression: String,
    expression_attribute_values: K,
) -> Option<HashMap<String, AttributeValue>> {
    let expression_attribute_values = serde_dynamo::to_item(expression_attribute_values)
        .expect("Unable to serialize DynamoDb key");

    let input = QueryInput {
        table_name: table_name.to_owned(),
        key_condition_expression: Some(key_condition_expression),
        expression_attribute_values: Some(expression_attribute_values),
        ..QueryInput::default()
    };

    dynamodb_client
        .query(input)
        .await
        .unwrap_or_else(|e| panic!("Unable to get rows from DynamoDb.{e:?}"))
        .items
        .and_then(|mut i| i.pop())
}

pub async fn put_item<I: Serialize>(dynamodb_client: &DynamoDbClient, table_name: &str, item: &I) {
    let _ = dynamodb_client
        .put_item(PutItemInput {
            table_name: table_name.to_string(),
            item: serde_dynamo::to_item(item).unwrap(),
            ..PutItemInput::default()
        })
        .await
        .expect("Item not inserted");
}

/// Creates a DynamoDB table
pub async fn create_table(client: &DynamoDbClient, table_definition: &str, table_name: String) {
    let mut input: CreateTableInput =
        serde_json::from_str(table_definition).expect("Could not load table definition");
    input.table_name = table_name;
    client
        .create_table(input)
        .await
        .expect("Could not create table");
}

/// Deletes a DynamoDB table
pub async fn delete_table(client: &DynamoDbClient, table_name: String) {
    client
        .delete_table(DeleteTableInput { table_name })
        .await
        .ok();

    // TODO: Refactor this to use in Drop. This failure is ignored because there are no seeds and
    // no table to delete
}

// A method that receives an `order_id` and uses it to scan table `order_status` to search the field `replaces` and return the matching items
pub async fn get_related_items(client: &DynamoDbClient, order_id: String) -> Vec<String> {
    let mut related_items: Vec<String> = Vec::new();

    let result = client
        .scan(rusoto_dynamodb::ScanInput {
            table_name: "order_status".to_string(),
            filter_expression: Some("replaces = :order_id".to_string()),
            expression_attribute_values: Some(std::collections::HashMap::from([(
                ":order_id".to_string(),
                rusoto_dynamodb::AttributeValue {
                    s: Some(order_id.clone()),
                    ..Default::default()
                },
            )])),
            ..Default::default()
        })
        .await
        .expect("Could not scan table");
    for item in result.items.unwrap_or_default() {
        let item: OrderStatus = serde_dynamo::from_item(item).unwrap();
        related_items.push(item.order_id.to_string());
    }

    related_items
}

/// Recreates a DynamoDB table
///
/// This function is used to remove all the rows of a DynamoDB table. The way of doing this is
/// deleting the table and recreating it with its definition. Other methods involves scan all the
/// table, this seems to be the fastests way.
pub async fn recreate_table(client: &DynamoDbClient, table_definition: &str, table_name: String) {
    delete_table(client, table_name.clone()).await;
    create_table(client, table_definition, table_name).await;
}
