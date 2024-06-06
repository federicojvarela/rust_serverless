#!/bin/bash

MAESTRO_URL=$1
DOMAIN_NAME=$2
MAESTRO_API_KEY=$3
MAESTRO_TENANT_NAME=$4
AE_TYPE_NAME=$5

echo "Logging in as Domain Admin"
source ../maestro/domain_login.sh
domain_login "$MAESTRO_URL" "$DOMAIN_NAME"

echo "Updating Domain Admin Roles"
# Make sure DomainAdmin has the correct roles
source ../maestro/update_domain_admin_role.sh
update_domain_admin_role "$MAESTRO_URL" "$MAESTRO_DOMAIN_ADMIN_SERVICE_NAME" "$MAESTRO_DOMAIN_ADMIN_SECRET"

echo "Creating Domain AE"
source ../maestro/create_domain_ae.sh
create_domain_ae "$MAESTRO_URL" "$DOMAIN_NAME" "$AE_TYPE_NAME"

echo "Creating passcode for AE"
source ../maestro/create_passcode_for_ae.sh
create_passcode_for_ae "$MAESTRO_URL" "$DOMAIN_NAME" "$AE_NAME"

echo "Adding public key for AE"
source ../maestro/add_public_key_for_ae.sh
add_public_key_for_ae "$MAESTRO_URL" "$MAESTRO_DOMAIN_AE_UPLOAD_PATH" "$MAESTRO_DOMAIN_AE_PASSCODE" "$AE_NAME"

# This MUST be the last thing we echo out so we can parse it in other script files. -n omits new line character
echo -n "$AE_NAME"