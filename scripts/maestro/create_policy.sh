#!/bin/bash

create_policy() {

    set -eu

    echo -n "#### CREATE DOMAIN LEVEL POLICY WITH MAESTRO ####"

    MAESTRO_URL=$1
    PRIVATE_KEY_FILE=$2
    BASE64_SERIALIZED_POLICY=$3

    # Grab the last line of the output from this (the signature)
    POLICY_REQUEST_SIGNATURE=$(../common/generate_signature.sh "$PRIVATE_KEY_FILE" "$BASE64_SERIALIZED_POLICY" | tail -n 1)
    echo "BASE64 ENCODED SIGNATURE: ${POLICY_REQUEST_SIGNATURE}"

    DECODED_POLICY_NAME=$(echo "$BASE64_SERIALIZED_POLICY" | base64 --decode | jq -r .policy_name)

    echo "#### Creating Policy with Name: ${DECODED_POLICY_NAME}"

    CREATE_POLICY_RESPONSE=$(curl --location "${MAESTRO_URL}/policy" \
    --header 'Content-Type: application/json' \
    --header "Authorization: Bearer $MAESTRO_ACCESS_TOKEN" \
    --data @- << EOF
      {
        "serialized_policy": "${BASE64_SERIALIZED_POLICY}",
        "display_name": "${DECODED_POLICY_NAME}",
        "signature": "${POLICY_REQUEST_SIGNATURE}"
      }
EOF
    )

    echo "#### RESPONSE: ${CREATE_POLICY_RESPONSE}"

    POLICY_NAME=$(echo "$CREATE_POLICY_RESPONSE" | jq -r .display_name)

    echo "Policy Created Successfully: ${POLICY_NAME}"

    set -eu
}