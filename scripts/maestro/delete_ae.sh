#!/bin/bash

delete_ae() {

    set -eu

    MAESTRO_URL=$1
    AE_NAME=$2

    echo "#### Beginning Policy Approver AE Deletion"

    DELETE_POLICY_APPROVER_STATUS_CODE=$(curl --write-out "%{http_code}\n" --silent --output /dev/null --location --request DELETE "${MAESTRO_URL}/authorizing_entity" \
    --header 'Content-Type: application/json' \
    --header "Authorization: Bearer $MAESTRO_ACCESS_TOKEN" \
    --data @- << EOF
      {
        "name": "${AE_NAME}"
      }
EOF
    )

    # This request has no response body, the only way we can tell if it failed is by checking the status code.
    # In other scripts we're using the `jq` command to try and parse the response body, the script exits if
    # the response body is not what we expect.
    if [ "$DELETE_POLICY_APPROVER_STATUS_CODE" != "200" ]; then
      echo "Error, DELETE /authorizing_entity response status code != 200, got ${DELETE_POLICY_APPROVER_STATUS_CODE}"
      exit 1
    fi

    echo "#### Deleted Policy Approver AE: ${AE_NAME}"
}