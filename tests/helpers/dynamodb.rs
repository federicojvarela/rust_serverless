use rusoto_dynamodb::{
    CreateTableInput, DeleteTableInput, DynamoDb, DynamoDbClient, GetItemInput, PutItemInput,
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

/// Recreates a DynamoDB table
///
/// This function is used to remove all the rows of a DynamoDB table. The way of doing this is
/// deleting the table and recreating it with its definition. Other methods involves scan all the
/// table, this seems to be the fastests way.
pub async fn recreate_table(client: &DynamoDbClient, table_definition: &str, table_name: String) {
    delete_table(client, table_name.clone()).await;
    create_table(client, table_definition, table_name).await;
}
