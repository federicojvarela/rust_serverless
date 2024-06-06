#!/bin/bash

create_ae_type() {

    set -eu

    echo "#### CREATING AN AUTHORIZING ENTITY TYPE WITH MAESTRO ####"

    MAESTRO_URL=$1
    AE_TYPE_NAME=$2

    MAESTRO_GET_DOMAIN_AE_TYPE_RESPONSE=$(curl -X GET "${MAESTRO_URL}/authorizing_entity/type" \
        --header 'Content-Type: application/json' \
        --header "Authorization: Bearer $MAESTRO_ACCESS_TOKEN")

    AE_TYPE_EXISTS=$(echo "$MAESTRO_GET_DOMAIN_AE_TYPE_RESPONSE" | jq -c "map(select(.name | contains(\"$AE_TYPE_NAME\")))")

    if [ "$AE_TYPE_EXISTS" != "[]" ]; then
        echo "The $AE_TYPE_NAME AE type already exists. Skipping creation..."
        return
    fi

    echo "#### Creating AE Type $AE_TYPE_NAME ####"

    MAESTRO_CREATE_DOMAIN_AE_TYPE_RESPONSE=$(curl -X POST "${MAESTRO_URL}/authorizing_entity/type" \
        --header 'Content-Type: application/json' \
        --header "Authorization: Bearer $MAESTRO_ACCESS_TOKEN" \
        --data @- << EOF
          {
            "name": "${AE_TYPE_NAME}",
            "description": "AE Type",
            "display_name": "${AE_TYPE_NAME}"
          }
EOF
        )

    echo "$MAESTRO_CREATE_DOMAIN_AE_TYPE_RESPONSE"
    TYPE_NAME=$(echo "$MAESTRO_CREATE_DOMAIN_AE_TYPE_RESPONSE" | jq -r .name)
    echo "Created Domain AE Type: $TYPE_NAME"

    set -eu
}