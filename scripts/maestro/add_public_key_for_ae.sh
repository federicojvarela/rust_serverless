#!/bin/bash

add_public_key_for_ae() {

    set -eu

    echo "#### ADD PUBLIC KEY FOR DOMAIN AE WITH MAESTRO ####"

    MAESTRO_URL=$1
    MAESTRO_DOMAIN_AE_UPLOAD_PATH=$2
    MAESTRO_DOMAIN_AE_PASSCODE=$3
    MAESTRO_DOMAIN_AE_NAME=$4

    ADD_PUBLIC_KEY_STATUS_CODE=$(curl --write-out "%{http_code}\n" --silent --output /dev/null --location --request PUT "${MAESTRO_URL}/authorizing_entity/public_key/${MAESTRO_DOMAIN_AE_UPLOAD_PATH}" \
    --header 'Content-Type: application/json' \
    --header "Authorization: Bearer $MAESTRO_ACCESS_TOKEN" \
    --data @- << EOF
      {
        "name": "${MAESTRO_DOMAIN_AE_NAME}",
        "passcode": "${MAESTRO_DOMAIN_AE_PASSCODE}",
        "public_key": "-----BEGIN PUBLIC KEY-----\nMFYwEAYHKoZIzj0CAQYFK4EEAAoDQgAEEfw/MOmtobnF36IKi6WcN/sSbP2nrdSE\n3bKZV9X0j+bukH19wqtyp+JC6OiKY5E8LQn5bWM7ihBy2+0Tl0mHVQ==\n-----END PUBLIC KEY-----"
      }
EOF
    )

    # This request has no response body, the only way we can tell if it failed is by checking the status code.
    # In other scripts we're using the `jq` command to try and parse the response body, the script exits if
    # the response body is not what we expect.
    if [ "$ADD_PUBLIC_KEY_STATUS_CODE" != "200" ]; then
      echo "Error, PUT /public_key response status code != 200, got ${ADD_PUBLIC_KEY_STATUS_CODE}"
      exit 1
    fi

    set -eu
}