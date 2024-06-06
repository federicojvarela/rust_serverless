#!/bin/bash

# Choosing not to wrap this in a function because bash makes sourcing functions inside other functions a pain

CLIENT_ID=$1
MAESTRO_API_KEY=$2
PRIVATE_KEY_PEM_FILE=$3
ENV_PREFIX=$4

MAESTRO_URL="https://maestro.preprod.boltforte.io"
MAESTRO_TENANT_NAME="Forte"

echo "#### STARTING MAESTRO SETUP ####"

source ../maestro/get_maestro_access_token.sh
get_maestro_access_token "$MAESTRO_URL" "$MAESTRO_API_KEY" "$MAESTRO_TENANT_NAME"

source ../maestro/create_client_domain.sh
create_client_domain "$CLIENT_ID" "$MAESTRO_URL"

source ../maestro/create_ae_type.sh
create_ae_type "$MAESTRO_URL" "PolicyApprover"
create_ae_type "$MAESTRO_URL" "domain_txn_approver"

echo "Creating Policy Approver AE"
# Create the Domain level "PolicyApprover" AE. It's a unique type of ae created when the domain is made
POLICY_APPROVER_AE=$(./maestro_setup_domain_ae.sh "$MAESTRO_URL" "$CLIENT_ID" "$MAESTRO_API_KEY" "$MAESTRO_TENANT_NAME" "PolicyApprover" | tail -n 1)

echo "Creating Domain Approver AE"
# Create a default txn approver for the Domain
DOMAIN_APPROVER_AE=$(./maestro_setup_domain_ae.sh "$MAESTRO_URL" "$CLIENT_ID" "$MAESTRO_API_KEY" "$MAESTRO_TENANT_NAME" "domain_txn_approver" | tail -n 1)
SOLO_APPROVER_POLICY=$(cat <<EOF
{
    "domain_approvals":
    {
        "optional":
        [],
        "required":
        []
    },
    "min_optional_approvals": 0,
    "policy_name": "${ENV_PREFIX}_DefaultTenantSoloApproval",
    "nonce": "1",
    "tenant_approvals":
    {
        "optional":
        [],
        "required":
        [
            "forte_waas_txn_approver"
        ]
    }
}
EOF
)
# Force remove temp file before creating in case it already exists
rm -f tmp_policy.json

echo "$SOLO_APPROVER_POLICY" > tmp_policy.json
SOLO_APPROVER_BASE64_SERIALIZED_POLICY=$(base64 -i tmp_policy.json | tr -d '\n')
rm tmp_policy.json

source ../maestro/domain_login.sh
domain_login "$MAESTRO_URL" "$DOMAIN_NAME"

source ../maestro/create_policy.sh
create_policy "$MAESTRO_URL" "$PRIVATE_KEY_PEM_FILE" "$SOLO_APPROVER_BASE64_SERIALIZED_POLICY"

POLICY_NAME=${ENV_PREFIX}_DualAutoApprovers

DUAL_APPROVER_POLICY="{\"policy_name\":\"${POLICY_NAME}\",\"nonce\":\"2\",\"domain_approvals\":{\"required\":[\"${DOMAIN_APPROVER_AE}\"],\"optional\":[]},\"tenant_approvals\":{\"required\":[\"forte_waas_txn_approver\"],\"optional\":[]},\"min_optional_approvals\":0}"
DUAL_APPROVER_POLICY_BASE64=$(echo -n "$DUAL_APPROVER_POLICY" | base64 | tr -d '\n')

create_policy "$MAESTRO_URL" "$PRIVATE_KEY_PEM_FILE" "$DUAL_APPROVER_POLICY_BASE64"

./dual_approver_setup.sh -e $ENV_PREFIX -c $CLIENT_ID

echo "#### MAESTRO SETUP COMPLETED ####"
