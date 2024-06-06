#!/bin/bash

update_domain_admin_role() {

    set -eu

    echo "#### ADD DOMAINPOLICYADMIN ROLE TO DOMAINADMIN WITH MAESTRO ####"

    MAESTRO_URL=$1
    MAESTRO_DOMAIN_ADMIN_SERVICE_NAME=$2
    MAESTRO_DOMAIN_ADMIN_SECRET=$3

    UPDATE_DOMAIN_ADMIN_ROLE_RESPONSE=$(curl --location --request PUT "${MAESTRO_URL}/api_user" \
    --header 'Content-Type: application/json' \
    --header "Authorization: Bearer $MAESTRO_ACCESS_TOKEN" \
    --data @- << EOF
      {
        "display_name": "${MAESTRO_DOMAIN_ADMIN_SERVICE_NAME}",
        "roles": ["DomainAdmin", "DomainPolicyAdmin", "AuthorizingAdmin"],
        "password": "${MAESTRO_DOMAIN_ADMIN_SECRET}",
        "username": "${MAESTRO_DOMAIN_ADMIN_SERVICE_NAME}"
      }
EOF
    )

    # Don't care about the result, but this will exit the script if the request didn't work
    echo "$UPDATE_DOMAIN_ADMIN_ROLE_RESPONSE" | jq -r .display_name

    set -eu
}