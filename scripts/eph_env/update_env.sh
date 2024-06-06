#!/bin/bash

SCRIPT_NAME=$(basename "$0")

set -eu

if [ -z "$1" ]; then
    echo "Error: No environment prefix specified as the first argument"
    echo "Usage: ${SCRIPT_NAME} PREFIX_ENV"
    echo "e.g. ${SCRIPT_NAME} wall-305 <-- (ticket number, recommended)"
    echo "e.g. ${SCRIPT_NAME} ad-1 <-- (developer initials)"
    exit 1
fi

source ../aws_config.sh

source ../../.ephemeral/$1-secrets.sh

cd ../../infrastructure/terraform

terraform init -backend-config=config/tfstate_bucket.hcl

WORKSPACE_EXISTS=$(terraform workspace list | grep $1 | tr -d '* ')

if [ "$WORKSPACE_EXISTS" = "$1" ]; then
    terraform workspace select $1
else
    terraform workspace new $1
fi

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

echo "Environment $1 updated"
