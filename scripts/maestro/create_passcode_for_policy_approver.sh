#!/bin/bash

# TODO: In Maestro v3.1 this is going to change, we'll need to create a Domain level AuthorizingAdmin to generate this passcode
create_passcode_for_policy_approver() {

    set -eu

    echo "#### CREATE PASSCODE FOR DOMAIN POLICY APPROVER WITH MAESTRO ####"

    MAESTRO_URL=$1
    POLICY_APPROVER_NAME=$2

    CREATE_PASSCODE_RESPONSE=$(curl --location --request PUT "${MAESTRO_URL}/authorizing_entity/${POLICY_APPROVER_NAME}/passcode" \
    --header 'Content-Type: application/json' \
    --header "Authorization: Bearer $MAESTRO_ACCESS_TOKEN")

    MAESTRO_DOMAIN_POLICY_APPROVER_PASSCODE=$(echo "$CREATE_PASSCODE_RESPONSE" | jq -r .passcode)
    MAESTRO_DOMAIN_POLICY_APPROVER_UPLOAD_PATH=$(echo "$CREATE_PASSCODE_RESPONSE" | jq -r .upload_path_parameter)

    set -eu
}