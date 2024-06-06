#!/bin/bash

domain_login() {

    set -eu

    echo "#### DOMAIN ADMIN AUTHENTICATION WITH MAESTRO ####"

    MAESTRO_URL=$1
    DOMAIN_NAME=$2

    MAESTRO_DOMAIN_ADMIN_SECRET="password" #FIXME: Maestro sets this by default on domain creation, we should add another script to update this value to something secure
    MAESTRO_DOMAIN_ADMIN_SERVICE_NAME="${DOMAIN_NAME}_admin"

    echo "#### obtaining access_token from Maestro at ${MAESTRO_URL} for DomainAdmin ####"

    MAESTRO_ACCESS_TOKEN_RESPONSE=$(curl --location "${MAESTRO_URL}/${MAESTRO_TENANT_NAME}/${DOMAIN_NAME}/login" \
    --header 'Content-Type: application/x-www-form-urlencoded' \
    --data-urlencode "username=${MAESTRO_DOMAIN_ADMIN_SERVICE_NAME}" \
    --data-urlencode "password=${MAESTRO_DOMAIN_ADMIN_SECRET}" \
    --data-urlencode 'grant_type=password'
    )

    printf "$MAESTRO_ACCESS_TOKEN_RESPONSE"
    MAESTRO_ACCESS_TOKEN=$(echo "$MAESTRO_ACCESS_TOKEN_RESPONSE" | jq -r .access_token)

    set -eu
}