#!/bin/bash

delete_client_domain() {

    set -eu

    MAESTRO_URL=$1
    DOMAIN_NAME=$2

    echo "#### Beginning Domain Deletion in Maestro"

    DELETE_DOMAIN_STATUS_CODE=$(curl --write-out "%{http_code}\n" --silent --output /dev/null --location --request DELETE "${MAESTRO_URL}/domain" \
        --header 'Content-Type: application/json' \
        --header "Authorization: Bearer $MAESTRO_ACCESS_TOKEN" \
        --data @- << EOF
          {
            "domain_name": "${DOMAIN_NAME}"
          }
EOF
        )

    # This request has no response body, checking the status code for logging purposes but it's okay to error here,
    # don't want to fail out the pipeline if the delete fails.
    if [ "$DELETE_DOMAIN_STATUS_CODE" != "200" ]; then
      echo "Error, DELETE /domain response status code != 200, got ${DELETE_DOMAIN_STATUS_CODE}"
    else
      echo "Deleted Domain: $DOMAIN_NAME"
    fi
}