#!/bin/bash

create_domain_ae() {

    set -eu

    echo "#### CREATING A DOMAIN LEVEL AUTHORIZING ENTITY WITH MAESTRO ####"

    MAESTRO_URL=$1
    DOMAIN_NAME=$2
    ENTITY_TYPE=$3

    echo "#### Creating AE for Domain $DOMAIN_NAME ####"
    DISPLAY_NAME="${DOMAIN_NAME} AE"
    NAME="${DOMAIN_NAME}_ae"
    if [ "$ENTITY_TYPE" == "PolicyApprover" ]; then
      DISPLAY_NAME="Domain PolicyApprover"
      NAME="${DOMAIN_NAME}_policy_approver"
    fi

    MAESTRO_CREATE_DOMAIN_AE_RESPONSE=$(curl -X POST "${MAESTRO_URL}/authorizing_entity" \
        --header 'Content-Type: application/json' \
        --header "Authorization: Bearer $MAESTRO_ACCESS_TOKEN" \
        --data @- << EOF
          {
            "name": "${NAME}",
            "entity_type": "${ENTITY_TYPE}",
            "description": "Domain Level AE",
            "display_name": "${DISPLAY_NAME}"
          }
EOF
        )

    echo "$MAESTRO_CREATE_DOMAIN_AE_RESPONSE"
    AE_NAME=$(echo "$MAESTRO_CREATE_DOMAIN_AE_RESPONSE" | jq -r .name)
    echo "Created Domain AE: $AE_NAME"

    set -eu
}