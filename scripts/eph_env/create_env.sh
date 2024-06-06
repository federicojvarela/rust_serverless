#!/bin/bash

SCRIPT_NAME=$(basename "$0")
REGION="us-west-2"

set -u

if [ -z "$1" ]; then
    echo "Error: No environment prefix specified as the first argument"
    echo "Usage: ${SCRIPT_NAME} PREFIX_ENV"
    echo "e.g. ${SCRIPT_NAME} wall-305 <-- (ticket number, recommended)"
    echo "e.g. ${SCRIPT_NAME} ad-1 <-- (developer initials)"
    exit 1
fi

if [[ $1 == *_* ]]; then
  echo "Ephemeral environment names can't contain underscore characters."
  exit 1
fi

source ../aws_config.sh

echo "Setting up secrets..."
source ./manage_secrets.sh create $1

source ../../.ephemeral/$1-secrets.sh

cd ../../infrastructure/terraform

terraform workspace select default

terraform init -backend-config=config/tfstate_bucket.hcl

WORKSPACE_EXISTS=$(terraform workspace list | grep $1 | tr -d '* ')

if [ "$WORKSPACE_EXISTS" = "$1" ]; then
    terraform workspace select $1
else
    terraform workspace new $1
fi


cd ../../scripts/eph_env

./build_lambdas.sh $1

cd ../../infrastructure/terraform

terraform apply -auto-approve \
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

cd ../../scripts/eph_env

set -eu

echo "Looking up ephemeral env client"
source ./lookup_client_id.sh "$1"

cd ../common

# Make sure the pem file isn't present before we start.
rm -f policy_private_key.pem

PRIVATE_KEY_ARN='arn:aws:secretsmanager:us-west-2:267505102317:secret:dev-mpc-compliance-private-key-DTIGFZ'
POLICY_PRIVATE_KEY=$(aws secretsmanager get-secret-value --secret-id ${PRIVATE_KEY_ARN} | jq -r ".SecretString")
echo -n "$POLICY_PRIVATE_KEY" >> policy_private_key.pem

# CLIENT_ID will be used as the name of the Domain to create in Maestro
./maestro_setup.sh "$CLIENT_ID" "$MAESTRO_API_KEY" "policy_private_key.pem" "$1"

# Delete our pem file
rm policy_private_key.pem

cd ../add_client
./add_policy_registry_to_dynamodb.sh "$1" "$CLIENT_ID" 11155111 "0x1c965d1241d0040a3fc2a030baeeefb35c155a4e" "$1_DualAutoApprovers" "$REGION"
./add_policy_registry_to_dynamodb.sh "$1" "$CLIENT_ID" 80002 "0x1c965d1241d0040a3fc2a030baeeefb35c155a4e" "$1_DualAutoApprovers" "$REGION"

cd ../eph_env

source ../authenticate.sh

source ./configure_e2e_settings.sh

echo "Environment $1 created"
