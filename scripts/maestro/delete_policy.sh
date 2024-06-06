#!/bin/bash

delete_policy() {

    set -eu

    MAESTRO_URL=$1
    POLICY_NAME=$2
    PRIVATE_KEY_FILE=$3
    BASE64_SERIALIZED_POLICY=$4

    echo "#### Beginning Policy Deletion"

    # This sets an env var called SIGNATURE that we can reference after
    POLICY_REQUEST_SIGNATURE=$(../common/generate_signature.sh "$PRIVATE_KEY_FILE" "$BASE64_SERIALIZED_POLICY" | tail -n 1)
    echo "BASE64 ENCODED SIGNATURE: ${POLICY_REQUEST_SIGNATURE}"

    GET_POLICY_STATUS_CODE=$(curl --write-out "%{http_code}\n" --silent --output /dev/null --location --request GET "${MAESTRO_URL}/policy/${POLICY_NAME}" \
    --header 'Accept: application/json' \
    --header "Authorization: Bearer $MAESTRO_ACCESS_TOKEN")

    # Check if the policy exists before trying to delete it.
    if [ "$GET_POLICY_STATUS_CODE" != "200" ]; then
      echo "Error, GET /policy/${POLICY_NAME} failed. Skip trying to delete it."
      exit 0
    fi

    DELETE_POLICY_STATUS_CODE=$(curl --write-out "%{http_code}\n" --silent --output /dev/null --location --request DELETE "${MAESTRO_URL}/policy" \
    --header 'Content-Type: application/json' \
    --header "Authorization: Bearer $MAESTRO_ACCESS_TOKEN" \
    --data @- << EOF
      {
        "payload": "$BASE64_SERIALIZED_POLICY",
        "signature": "$POLICY_REQUEST_SIGNATURE"
      }
EOF
    )

    # This request has no response body, the only way we can tell if it failed is by checking the status code.
    # In other scripts we're using the `jq` command to try and parse the response body, the script exits if
    # the response body is not what we expect.
    if [ "$DELETE_POLICY_STATUS_CODE" != "200" ]; then
      echo "Error, DELETE /policy response status code != 200, got ${DELETE_POLICY_STATUS_CODE}"
      exit 0
    fi

    echo "#### Deleted Policy: ${POLICY_NAME}"
}