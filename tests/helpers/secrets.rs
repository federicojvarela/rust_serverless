use rusoto_secretsmanager::{
    CreateSecretRequest, DeleteSecretRequest, SecretsManager, SecretsManagerClient,
};
use uuid::Uuid;

pub async fn create_string_secret(
    secrets_manager_client: &SecretsManagerClient,
    secret_name: &str,
    secret_value: &str,
) {
    secrets_manager_client
        .create_secret(CreateSecretRequest {
            name: secret_name.to_owned(),
            secret_string: Some(secret_value.to_owned()),
            client_request_token: Some(Uuid::new_v4().to_string()),
            ..Default::default()
        })
        .await
        .expect("there was an error creating the secret");
}

pub async fn remove_secret(secrets_manager_client: &SecretsManagerClient, secret_name: &str) {
    secrets_manager_client
        .delete_secret(DeleteSecretRequest {
            force_delete_without_recovery: Some(true),
            secret_id: secret_name.to_owned(),
            ..Default::default()
        })
        .await
        .ok();
}

pub async fn recreate_string_secret(
    secrets_manager_client: &SecretsManagerClient,
    secret_name: &str,
    secret_value: &str,
) {
    remove_secret(secrets_manager_client, secret_name).await;
    create_string_secret(secrets_manager_client, secret_name, secret_value).await;
}
