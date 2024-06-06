#!/bin/bash

set -eu

SCRIPT_NAME=$(basename "$0")

if [ -z "$1" ]; then
  echo "Error: No environment prefix specified as the first argument"
  echo "Usage: ${SCRIPT_NAME} PREFIX_ENV"
  echo "e.g. ${SCRIPT_NAME} wall-305 <-- (ticket number, recommended)"
  echo "e.g. ${SCRIPT_NAME} ad-1 <-- (developer initials)"
  exit 1
fi

source ../aws_config.sh

SECRETS_FILE="../../.ephemeral/$1-secrets.sh"
MANIFEST_FILE="../../.ephemeral/$1-manifest.json"

if [ -f "$SECRETS_FILE" ]; then
  source $SECRETS_FILE

  export MAESTRO_API_KEY=$(aws secretsmanager get-secret-value --secret-id ${secret_arn_maestro_api_key} | jq -r ".SecretString")

  echo "Looking up ephemeral env client"
  source ./lookup_client_id.sh "$1"

  # Force remove .pem file in case it exists (something broke on a previous run)
  rm -f policy_private_key.pem

  PRIVATE_KEY_ARN='arn:aws:secretsmanager:us-west-2:267505102317:secret:dev-mpc-compliance-private-key-DTIGFZ'
  POLICY_PRIVATE_KEY=$(aws secretsmanager get-secret-value --secret-id ${PRIVATE_KEY_ARN} | jq -r ".SecretString")
  echo -n "$POLICY_PRIVATE_KEY" >> policy_private_key.pem

  # CLIENT_ID will be used as the name of the Domain to delete in Maestro
  ../common/maestro_teardown.sh "$CLIENT_ID" "$MAESTRO_API_KEY" "policy_private_key.pem" "$1"

  # Delete our pem file
  rm policy_private_key.pem

  cd ../../infrastructure/terraform

  if [ -f "$MANIFEST_FILE" ]; then
    terraform destroy -auto-approve \
      -var-file ../values/ephemeral.tfvars \
      -var="secret_arn_maestro_api_key=$secret_arn_maestro_api_key" \
      -var="secret_arn_launchdarkly_sdk_key=$secret_arn_launchdarkly_sdk_key" \
      -var="secret_arn_mpc_compliance_private_key=$secret_arn_mpc_compliance_private_key" \
      -var="secret_name_mpc_compliance_private_key=$secret_name_mpc_compliance_private_key" \
      -var="secret_arn_alchemy_ethereum_mainnet_api_key=$secret_arn_alchemy_ethereum_mainnet_api_key" \
      -var="secret_arn_alchemy_ethereum_sepolia_api_key=$secret_arn_alchemy_ethereum_sepolia_api_key" \
      -var="secret_arn_alchemy_polygon_mainnet_api_key=$secret_arn_alchemy_polygon_mainnet_api_key" \
      -var="secret_arn_alchemy_polygon_amoy_api_key=$secret_arn_alchemy_polygon_amoy_api_key" \
      -var="prefix_env=$1" \
      -var="lambda_artifacts_json=$(cat ../../.ephemeral/$1-manifest.json)"
    terraform workspace select default
    cd ../../scripts/eph_env
  else
    echo "ERROR: The JSON manifest file ($MANIFEST_FILE) is missing. Run './build_lambdas.sh $1' again to restore it"
    exit 1
  fi

else
  echo "ERROR: The secrets file ($SECRETS_FILE) is missing. Are you sure this environment hasn't already been destroyed?"
  echo "Run './manage_secrets.sh restore $1' to recreate the secrets file"
  exit 1
fi

echo "Deleting secrets..."
./manage_secrets.sh delete $1

echo "Deleting files on S3..."
aws s3 rm s3://mpc-ephemeral-envs/$1 --recursive

echo "Deleting local JSON manifest..."
rm ../../.ephemeral/$1-manifest.json

ENV_FILE="../../e2e/env_files/.env.test.e2e.$1.eph"

if [ -e "$ENV_FILE" ]; then
  rm ../../e2e/env_files/.env.test.e2e.$1.eph
else
  echo "Error: .env.test.e2e.$1.eph file not found!"
fi

./force_destroy_env.sh -p $1

echo "Environment $1 destroyed"
