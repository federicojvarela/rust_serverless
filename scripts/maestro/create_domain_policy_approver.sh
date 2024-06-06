#!/bin/bash

create_domain_policy_approver() {

    set -eu

    echo "#### CREATE DOMAIN LEVEL POLICY APPROVER WITH MAESTRO ####"

    MAESTRO_URL=$1
    DOMAIN_NAME=$2

    CREATE_DOMAIN_POLICY_APPROVER_RESPONSE=$(curl --location "${MAESTRO_URL}/authorizing_entity" \
    --header 'Content-Type: application/json' \
    --header "Authorization: Bearer $MAESTRO_ACCESS_TOKEN" \
    --data @- << EOF
      {
        "name": "${DOMAIN_NAME}_policy_approver",
        "entity_type": "PolicyApprover",
        "description": "PolicyApprover AE",
        "display_name": "PolicyApprover"
      }
EOF
    )

    POLICY_APPROVER_NAME=$(echo "$CREATE_DOMAIN_POLICY_APPROVER_RESPONSE" | jq -r .name)

    echo "Policy Approver Created Successfully: ${POLICY_APPROVER_NAME}"

    set -eu
}