#!/bin/bash

SCRIPT_NAME=$(basename "$0")

set -eu

ERROR_FOUND=0

if [ -z "$1" ]; then
    echo "Error: No secrets management command was specified as the first argument"
    ERROR_FOUND=1
fi

if [ -z "$2" ]; then
    echo "Error: No prefix env was specified as the second argument"
    ERROR_FOUND=1
fi

if [ $ERROR_FOUND -eq 1 ]; then
    SCRIPT_NAME=$(basename "$0")
    echo "Usage: ${SCRIPT_NAME} [create|delete] prefix_env"
    echo "e.g. ${SCRIPT_NAME} create wall-305"
    echo "e.g. ${SCRIPT_NAME} delete wall-305"
    exit 1
fi

source ../aws_config.sh

MAESTRO_API_KEY_ARN='arn:aws:secretsmanager:us-west-2:267505102317:secret:dev-mpc-maestro-api-key-QM9Akk'
COMPLIANCE_PRIVATE_KEY_ARN='arn:aws:secretsmanager:us-west-2:267505102317:secret:dev-mpc-compliance-private-key-DTIGFZ'
LD_SDK_KEY_ARN='arn:aws:secretsmanager:us-west-2:267505102317:secret:dev-mpc_launchdarkly_sdk_key-1NkkEh'
ETHEREUM_MAINNET_API_KEY_ARN='arn:aws:secretsmanager:us-west-2:267505102317:secret:dev-mpc_alchemy_ethereum_mainnet_api_key-y0QoRR'
ETHEREUM_SEPOLIA_API_KEY_ARN='arn:aws:secretsmanager:us-west-2:267505102317:secret:dev-mpc_alchemy_ethereum_sepolia_api_key-HVYrdF'
POLYGON_MAINNET_API_KEY_ARN='arn:aws:secretsmanager:us-west-2:267505102317:secret:dev-mpc_alchemy_polygon_mainnet_api_key-N0TAif'
POLYGON_AMOY_API_KEY_ARN='arn:aws:secretsmanager:us-west-2:267505102317:secret:dev-mpc_alchemy_polygon_amoy_api_key-fpBbDx'

SECRETS_EXPORT_FILE="../../.ephemeral/$2-secrets.sh"
mkdir -p ../../.ephemeral

if [ "$1" == "create" ] || [ "$1" == "restore" ]; then
    MAESTRO_API_KEY=$(aws secretsmanager get-secret-value --secret-id ${MAESTRO_API_KEY_ARN} | jq -r ".SecretString")
    COMPLIANCE_PRIVATE_KEY=$(aws secretsmanager get-secret-value --secret-id ${COMPLIANCE_PRIVATE_KEY_ARN} | jq -r ".SecretString")
    LD_SDK_KEY=$(aws secretsmanager get-secret-value --secret-id ${LD_SDK_KEY_ARN} | jq -r ".SecretString")
    ETHEREUM_MAINNET_API_KEY=$(aws secretsmanager get-secret-value --secret-id ${ETHEREUM_MAINNET_API_KEY_ARN} | jq -r ".SecretString")
    ETHEREUM_SEPOLIA_API_KEY=$(aws secretsmanager get-secret-value --secret-id ${ETHEREUM_SEPOLIA_API_KEY_ARN} | jq -r ".SecretString")
    POLYGON_MAINNET_API_KEY=$(aws secretsmanager get-secret-value --secret-id ${POLYGON_MAINNET_API_KEY_ARN} | jq -r ".SecretString")
    POLYGON_AMOY_API_KEY=$(aws secretsmanager get-secret-value --secret-id ${POLYGON_AMOY_API_KEY_ARN} | jq -r ".SecretString")

    if [ -f "$SECRETS_EXPORT_FILE" ]; then
        echo "WARNING: skipping secrets creation or restoration since the secrets file ($SECRETS_EXPORT_FILE) already exists"
        return
    fi
    
    if [ "$1" == "restore" ]; then
        MAESTRO_API_KEY_JSON=$(aws secretsmanager restore-secret --secret-id $2-mpc-maestro-api-key)

        COMPLIANCE_PRIVATE_KEY_JSON=$(aws secretsmanager restore-secret --secret-id $2-mpc-compliance-private-key)

        LD_SDK_KEY_JSON=$(aws secretsmanager restore-secret --secret-id $2-mpc_launchdarkly_sdk_key)

        ETHEREUM_MAINNET_API_KEY_JSON=$(aws secretsmanager restore-secret --secret-id $2-mpc_alchemy_ethereum_mainnet_api_key)

        ETHEREUM_SEPOLIA_API_KEY_JSON=$(aws secretsmanager restore-secret --secret-id $2-mpc_alchemy_ethereum_sepolia_api_key)

        POLYGON_MAINNET_API_KEY_JSON=$(aws secretsmanager restore-secret --secret-id $2-mpc_alchemy_polygon_mainnet_api_key)

        POLYGON_AMOY_API_KEY_JSON=$(aws secretsmanager restore-secret --secret-id $2-mpc_alchemy_polygon_amoy_api_key)

        echo "Secrets restored"
    else
        MAESTRO_API_KEY_JSON=$(aws secretsmanager create-secret \
            --name $2-mpc-maestro-api-key \
            --secret-string "${MAESTRO_API_KEY}")

        COMPLIANCE_PRIVATE_KEY_JSON=$(aws secretsmanager create-secret \
            --name $2-mpc-compliance-private-key \
            --secret-string "${COMPLIANCE_PRIVATE_KEY}")

        LD_SDK_KEY_JSON=$(aws secretsmanager create-secret \
            --name $2-mpc_launchdarkly_sdk_key \
            --secret-string "${LD_SDK_KEY}")

        ETHEREUM_MAINNET_API_KEY_JSON=$(aws secretsmanager create-secret \
            --name $2-mpc_alchemy_ethereum_mainnet_api_key \
            --secret-string "${ETHEREUM_MAINNET_API_KEY}")

        ETHEREUM_SEPOLIA_API_KEY_JSON=$(aws secretsmanager create-secret \
            --name $2-mpc_alchemy_ethereum_sepolia_api_key \
            --secret-string "${ETHEREUM_SEPOLIA_API_KEY}")

        POLYGON_MAINNET_API_KEY_JSON=$(aws secretsmanager create-secret \
            --name $2-mpc_alchemy_polygon_mainnet_api_key \
            --secret-string "${POLYGON_MAINNET_API_KEY}")

        POLYGON_AMOY_API_KEY_JSON=$(aws secretsmanager create-secret \
            --name $2-mpc_alchemy_polygon_amoy_api_key \
            --secret-string "${POLYGON_AMOY_API_KEY}")

        echo "Secrets created"
    fi

    MAESTRO_API_KEY_ARN=$(echo $MAESTRO_API_KEY_JSON | jq -r '.ARN')
    COMPLIANCE_PRIVATE_KEY_ARN=$(echo $COMPLIANCE_PRIVATE_KEY_JSON | jq -r '.ARN')
    LD_SDK_KEY_ARN=$(echo $LD_SDK_KEY_JSON | jq -r '.ARN')
    ETHEREUM_MAINNET_API_KEY_ARN=$(echo $ETHEREUM_MAINNET_API_KEY_JSON | jq -r '.ARN')
    ETHEREUM_SEPOLIA_API_KEY_ARN=$(echo $ETHEREUM_SEPOLIA_API_KEY_JSON | jq -r '.ARN')
    POLYGON_MAINNET_API_KEY_ARN=$(echo $POLYGON_MAINNET_API_KEY_JSON | jq -r '.ARN')
    POLYGON_AMOY_API_KEY_ARN=$(echo $POLYGON_AMOY_API_KEY_JSON | jq -r '.ARN')

    echo "secret_arn_maestro_api_key=${MAESTRO_API_KEY_ARN}" >$SECRETS_EXPORT_FILE
    echo "secret_arn_launchdarkly_sdk_key=${LD_SDK_KEY_ARN}" >>$SECRETS_EXPORT_FILE
    echo "secret_arn_mpc_compliance_private_key=${COMPLIANCE_PRIVATE_KEY_ARN}" >>$SECRETS_EXPORT_FILE
    echo "secret_name_mpc_compliance_private_key=$2-mpc-compliance-private-key" >>$SECRETS_EXPORT_FILE
    echo "secret_arn_alchemy_ethereum_mainnet_api_key=${ETHEREUM_MAINNET_API_KEY_ARN}" >>$SECRETS_EXPORT_FILE
    echo "secret_arn_alchemy_ethereum_sepolia_api_key=${ETHEREUM_SEPOLIA_API_KEY_ARN}" >>$SECRETS_EXPORT_FILE
    echo "secret_arn_alchemy_polygon_mainnet_api_key=${POLYGON_MAINNET_API_KEY_ARN}" >>$SECRETS_EXPORT_FILE
    echo "secret_arn_alchemy_polygon_amoy_api_key=${POLYGON_AMOY_API_KEY_ARN}" >>$SECRETS_EXPORT_FILE

    aws s3 cp $SECRETS_EXPORT_FILE s3://mpc-ephemeral-envs/$2/configs/secrets.sh

elif [ "$1" == "delete" ]; then
    if [ ! -f "$SECRETS_EXPORT_FILE" ]; then
        echo "ERROR: Cannot delete secrets since the secrets file ($SECRETS_EXPORT_FILE) doesn't exist"
        exit 1
    fi
    source $SECRETS_EXPORT_FILE
    aws secretsmanager delete-secret --secret-id $secret_arn_maestro_api_key

    aws secretsmanager delete-secret --secret-id $secret_arn_launchdarkly_sdk_key

    aws secretsmanager delete-secret --secret-id $secret_arn_mpc_compliance_private_key

    aws secretsmanager delete-secret --secret-id $secret_arn_alchemy_ethereum_mainnet_api_key

    aws secretsmanager delete-secret --secret-id $secret_arn_alchemy_ethereum_sepolia_api_key

    aws secretsmanager delete-secret --secret-id $secret_arn_alchemy_polygon_mainnet_api_key

    aws secretsmanager delete-secret --secret-id $secret_arn_alchemy_polygon_amoy_api_key

    rm -f $SECRETS_EXPORT_FILE
    echo "Secrets deleted"
else
    echo "Command provided not supported: \"$1\""
fi
