#!/bin/bash

if [ "$#" -ne 2 ]; then
  SCRIPT_NAME=$(basename "$0")
  echo "Usage: ${SCRIPT_NAME} ae_name client_id"
  echo "e.g. ${SCRIPT_NAME} T1231234  4lak27h7hvtoga94gse40btrrm"
  exit 1
fi

source ./set_env_vars.sh

ANAME=$1
CLIENT_ID=$2
AWS_REGION="us-west-2"

MAESTRO_TENANT_NAME="Forte"


echo "#### Creating SQS Queues"
source ../common/create_ae_sqs_queues.sh "$ENV_PREFIX" "$CLIENT_ID" "$AWS_REGION"

echo "#### Creating event bridge pipe for the response queue"
../common/create_event_bridge_pipe_for_response_queue.sh "$ENV_PREFIX" "$CLIENT_ID" "$SQS_RESPONSE_QUEUE_ARN" "$AWS_REGION"

echo "#### Adding mapping to the AEs table for request queue"
../common/add_ae_record_to_dynamo.sh "$ENV_PREFIX" "${ANAME}_ae" "$SQS_REQUEST_QUEUE_URL" "$SQS_REQUEST_QUEUE_ARN" "$AWS_REGION"


echo "#### Get Maestro API Key ####"
SECRET_NAME="${ENV_PREFIX}-mpc-maestro-admin-credentials"
SECRETS_MANAGER_RESPONSE=$(aws secretsmanager get-secret-value --secret-id "$SECRET_NAME")
SECRET_STRING=$(echo "$SECRETS_MANAGER_RESPONSE" | jq -r .SecretString)
MAESTRO_API_KEY=$(echo "$SECRET_STRING" | jq -r .api_secret)

echo "#### Initiating Maestro domain login ####"
source ../maestro/domain_login.sh
domain_login "$MAESTRO_URL" "$CLIENT_ID"

echo "Updating Domain Admin Roles"
# Make sure DomainAdmin has the correct roles
source ../maestro/update_domain_admin_role.sh
update_domain_admin_role "$MAESTRO_URL" "$MAESTRO_DOMAIN_ADMIN_SERVICE_NAME" "$MAESTRO_DOMAIN_ADMIN_SECRET"

echo "Create domain AE"
source ../maestro/create_domain_ae.sh
create_domain_ae "$MAESTRO_URL" "$ANAME" "domain_txn_approver"

echo "Creating passcode for AE"
source ../maestro/create_passcode_for_ae.sh
create_passcode_for_ae "$MAESTRO_URL" "$CLIENT_ID" "$AE_NAME"


echo "Adding public key for AE"
source ../maestro/add_public_key_for_ae.sh
add_public_key_for_ae "$MAESTRO_URL" "$MAESTRO_DOMAIN_AE_UPLOAD_PATH" "$MAESTRO_DOMAIN_AE_PASSCODE" "$AE_NAME"

