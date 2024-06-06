#!/bin/bash

SCRIPT_NAME=$(basename "$0")

if [ -z "$1" ]; then
  echo "Usage: ${SCRIPT_NAME} client_name"
  echo "e.g. ${SCRIPT_NAME} facepalm_nile_pear_flixnet_shmoogle"
  exit 1
fi

source ./set_env_vars.sh

#### global vars ####
SQS_REQUEST_QUEUE_URL=""
SQS_REQUEST_QUEUE_ARN=""
SQS_RESPONSE_QUEUE_URL=""
SQS_RESPONSE_QUEUE_ARN=""

main() {

  CLIENT_ID=""
  CLIENT_SECRET=""
  CLIENT_AUTH_TOKEN=""
  CLIENT_NAME="$1"
  echo "You want to create some AWS infrastructure for a new approver for the following client:"
  echo "CLIENT_NAME: $CLIENT_NAME"
  read -r -p "Are you sure? [y/n] " response
  if ! [[ "$response" =~ ^([yY][eE][sS]|[yY])$ ]]; then
    echo "#### No changes were made ####"
    exit 1
  fi

  echo "#### Authenticating... ####"
  source ../aws_config.sh
  source ../authenticate.sh "$ENV_PREFIX"

  create_cognito_app_client "$CLIENT_NAME"

  SECRET_NAME="${ENV_PREFIX}-mpc-maestro-admin-credentials"
  SECRETS_MANAGER_RESPONSE=$(aws secretsmanager get-secret-value --secret-id "$SECRET_NAME")
  SECRET_STRING=$(echo "$SECRETS_MANAGER_RESPONSE" | jq -r .SecretString)
  MAESTRO_API_KEY=$(echo "$SECRET_STRING" | jq -r .api_secret)

  POLICY_PRIVATE_KEY=$(aws secretsmanager get-secret-value --secret-id ${PRIVATE_KEY_ARN} | jq -r ".SecretString")

  # Force remove .pem file in case it exists (something broke on a previous run)
  rm -f policy_private_key.pem

  echo -n "$POLICY_PRIVATE_KEY" >>policy_private_key.pem

  ../common/maestro_setup.sh "$CLIENT_ID" "$MAESTRO_API_KEY" "policy_private_key.pem" "$ENV_PREFIX"

  # Delete our pem file
  rm policy_private_key.pem

  generate_auth_token "$CLIENT_ID" "$CLIENT_SECRET"

  echo ""
  echo "#### Assuming all went well, here is what we have: ####"
  echo ""
  echo "CLIENT_NAME:   $CLIENT_NAME"
  echo "CLIENT_ID:     $CLIENT_ID"
  echo "CLIENT_SECRET: $CLIENT_SECRET"
  echo "AUTH_TOKEN:    $CLIENT_AUTH_TOKEN" | tr -d '\n'
  echo ""
  echo "SQS_REQUEST_QUEUE_URL:  $SQS_REQUEST_QUEUE_URL"
  echo "SQS_REQUEST_QUEUE_ARN:  $SQS_REQUEST_QUEUE_ARN"
  echo "SQS_RESPONSE_QUEUE_URL: $SQS_RESPONSE_QUEUE_URL"
  echo "SQS_RESPONSE_QUEUE_ARN: $SQS_RESPONSE_QUEUE_ARN"
  echo ""
  echo "#### The setup is complete ####"
}

create_cognito_app_client() {
  CLIENT_NAME=$1

  #https://awscli.amazonaws.com/v2/documentation/api/latest/reference/cognito-idp/create-user-pool-client.html
  echo "#### creating cognito app client ####"
  CREATE_APP_CLIENT_RESULT=$(aws cognito-idp create-user-pool-client \
    --user-pool-id "$USER_POOL_ID" \
    --client-name "$CLIENT_NAME" \
    --generate-secret \
    --read-attributes "[\"name\"]" \
    --write-attributes "[\"email\"]" \
    --explicit-auth-flows "[\"ALLOW_REFRESH_TOKEN_AUTH\"]" \
    --allowed-o-auth-flows-user-pool-client \
    --allowed-o-auth-flows "[\"client_credentials\"]" \
    --allowed-o-auth-scopes "[\"${GATEWAY_URL}/create_key\", \"${GATEWAY_URL}/sign\", \"${GATEWAY_URL}/get_order_status\", \"${GATEWAY_URL}/export_openapi\", \"${GATEWAY_URL}/get_historical_fees\", \"${GATEWAY_URL}/get_gas_price_prediction\", \"${GATEWAY_URL}/tokens_ft\", \"${GATEWAY_URL}/tokens_native\", \"${GATEWAY_URL}/tokens_nft\", \"${GATEWAY_URL}/sponsored\"]")

  echo "$CREATE_APP_CLIENT_RESULT"
  CLIENT_ID=$(echo "$CREATE_APP_CLIENT_RESULT" | jq -r .UserPoolClient.ClientId)
  CLIENT_SECRET=$(echo "$CREATE_APP_CLIENT_RESULT" | jq -r .UserPoolClient.ClientSecret)
}

generate_auth_token() {
  CLIENT_ID=$1
  CLIENT_SECRET=$2
  CLIENT_AUTH_TOKEN=$(echo -n "${CLIENT_ID}:${CLIENT_SECRET}" | openssl base64)
}

main $1 $2 $3
exit
