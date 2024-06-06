#!/bin/bash

create_passcode_for_ae() {

    set -eu

    echo "#### CREATE PASSCODE FOR DOMAIN LEVEL AE WITH MAESTRO ####"

    MAESTRO_URL=$1
    DOMAIN_NAME=$2
    AE_NAME=$3

    CREATE_PASSCODE_RESPONSE=$(curl --location --request PUT "${MAESTRO_URL}/authorizing_entity/${AE_NAME}/passcode" \
    --header 'Content-Type: application/json' \
    --header "Authorization: Bearer $MAESTRO_ACCESS_TOKEN")

    MAESTRO_DOMAIN_AE_PASSCODE=$(echo "$CREATE_PASSCODE_RESPONSE" | jq -r .passcode)
    MAESTRO_DOMAIN_AE_UPLOAD_PATH=$(echo "$CREATE_PASSCODE_RESPONSE" | jq -r .upload_path_parameter)

    set -eu
}