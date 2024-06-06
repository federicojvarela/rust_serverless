#!/bin/bash

# Choosing not to wrap this in a function because bash makes sourcing functions inside other functions a pain

DOMAIN_NAME=$1
MAESTRO_API_KEY=$2
PRIVATE_KEY_PEM_FILE=$3
ENV_PREFIX=$4

MAESTRO_URL="https://maestro.preprod.boltforte.io"
MAESTRO_TENANT_NAME="Forte"

echo "#### STARTING MAESTRO TEARDOWN ####"

source ../maestro/domain_login.sh
domain_login "$MAESTRO_URL" "$DOMAIN_NAME"

# payload to delete a policy
SOLO_APPROVER_POLICY="{\"policy_name\": \"${ENV_PREFIX}_DefaultTenantSoloApproval\",\"nonce\": \"3\"}"
BASE64_SERIALIZED_POLICY=$(echo -n "$SOLO_APPROVER_POLICY" | base64 | tr -d '\n')

source ../maestro/delete_policy.sh

delete_policy "$MAESTRO_URL" "${ENV_PREFIX}_DefaultTenantSoloApproval" "$PRIVATE_KEY_PEM_FILE" "$BASE64_SERIALIZED_POLICY"

POLICY_NAME=${ENV_PREFIX}_DualAutoApprovers
DUAL_APPROVER_POLICY="{\"policy_name\":\"${POLICY_NAME}\",\"nonce\":\"4\"}"
DUAL_APPROVER_POLICY_BASE64=$(echo -n "$DUAL_APPROVER_POLICY" | base64 | tr -d '\n')
delete_policy "$MAESTRO_URL" "$POLICY_NAME" "$PRIVATE_KEY_PEM_FILE" "$DUAL_APPROVER_POLICY_BASE64"

# TODO: Should GET and loop over these as well
source ../maestro/delete_ae.sh
delete_ae "$MAESTRO_URL" "${DOMAIN_NAME}_policy_approver"
delete_ae "$MAESTRO_URL" "${DOMAIN_NAME}_ae"

source ../maestro/get_maestro_access_token.sh
get_maestro_access_token "$MAESTRO_URL" "$MAESTRO_API_KEY" "$MAESTRO_TENANT_NAME"

source ../maestro/delete_client_domain.sh
delete_client_domain "$MAESTRO_URL" "$DOMAIN_NAME"

echo "#### MAESTRO TEARDOWN COMPLETE"
