#!/bin/bash

create_client_domain() {

    set -eu

    echo "#### CREATING A CLIENT DOMAIN WITH MAESTRO ####"

    CLIENT_ID=$1
    MAESTRO_URL=$2

    echo "#### Creating Domain for $CLIENT_ID ####"

    MAESTRO_CREATE_DOMAIN_RESPONSE=$(curl -X POST "${MAESTRO_URL}/domain" \
      --header 'Content-Type: application/json' \
      --header "Authorization: Bearer $MAESTRO_ACCESS_TOKEN" \
      --data @- << EOF
        {
          "domain_name": "${CLIENT_ID}",
          "description": "client_id ${CLIENT_ID}"
        }
EOF
      )

    DOMAIN_NAME=$(echo "$MAESTRO_CREATE_DOMAIN_RESPONSE" | jq -r .domain_name)
    echo "Created domain $DOMAIN_NAME"

    set -eu
}