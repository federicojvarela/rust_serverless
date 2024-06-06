#!/bin/bash

get_maestro_access_token() {

    set -eu

    echo "#### AUTHENTICATION WITH MAESTRO ####"

    MAESTRO_URL=$1
    MAESTRO_API_KEY=$2
    MAESTRO_TENANT_NAME=$3
    MAESTRO_SERVICE_NAME="Forte_Administrator"

    echo "#### obtaining access_token from Maestro at ${MAESTRO_URL} ####"

    MAESTRO_ACCESS_TOKEN_RESPONSE=$(curl --location "${MAESTRO_URL}/${MAESTRO_TENANT_NAME}/login" \
        --header 'Content-Type: application/x-www-form-urlencoded' \
        --data-urlencode "username=${MAESTRO_SERVICE_NAME}" \
        --data-urlencode "password=${MAESTRO_API_KEY}" \
        --data-urlencode 'grant_type=password'
        )

    MAESTRO_ACCESS_TOKEN=$(echo "$MAESTRO_ACCESS_TOKEN_RESPONSE" | jq -r .access_token)
    echo "#### authentication successful"

    set -eu
}