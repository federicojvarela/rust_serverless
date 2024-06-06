#!/bin/bash

SCRIPT_NAME=$(basename "$0")

if [ "$#" -ne 4 ]; then
  SCRIPT_NAME=$(basename "$0")
  echo "Usage: ${SCRIPT_NAME} policy_name domain_name domain_approvals tenant_approvals"
  echo "e.g TetsPolicy1 4lak27h7hvtoga94gse40btrrm 'TestApproval1,TestApproval2' "
  exit 1
fi

source ./set_env_vars.sh

POLICY_NAME=$1
DOMAIN_NAME=$2
DOMAIN_APPROVALS=$3
TENANT_APPROVALS=$4

MAESTRO_TENANT_NAME="Forte"

OLD_IFS=$IFS

IFS=','

read -ra domain_arr <<< "$DOMAIN_APPROVALS"
read -ra tenant_arr <<< "$TENANT_APPROVALS"


IFS=$OLD_IFS

echo "Domain Approvals: $domain_arr"

echo "#### Initiating Maestro domain login ####"
source ../maestro/domain_login.sh
domain_login "$MAESTRO_URL" "$DOMAIN_NAME"

echo "#### Creating Maestro policy ####"

policy_template='{"domain_approvals":{"optional":[],"required":[]},"min_optional_approvals":0,"policy_name":"%s","tenant_approvals":{"optional":[],"required":[]}}'

policy_json=$(printf "$policy_template"  "$POLICY_NAME")


domain_approvals_arr_json=$(printf '%s\n' "${domain_arr[@]}" | jq -R . | jq -s .)
tenant_approvals_arr_json=$(printf '%s\n' "${tenant_arr[@]}" | jq -R . | jq -s .)


echo "domain_approvals_arr_json:$domain_approvals_arr_json"

policy_json=$( echo "$policy_json" | jq --argjson arr "$domain_approvals_arr_json" '.domain_approvals += {required: $arr}')
policy_json=$( echo "$policy_json" | jq --argjson arr "$tenant_approvals_arr_json" '.tenant_approvals += {required: $arr}')


echo "policy json: $policy_json"
BASE64_SERIALIZED_POLICY=$(echo $policy_json | base64 | tr -d '\n')

echo "BASE64 ENCODED SERIALIZED POLICY: $BASE64_SERIALIZED_POLICY"

# Force remove .pem file in case it exists (something broke on a previous run)
rm -f policy_private_key.pem

POLICY_PRIVATE_KEY=$(aws secretsmanager get-secret-value --secret-id ${PRIVATE_KEY_ARN} | jq -r ".SecretString")
echo -n "$POLICY_PRIVATE_KEY" >> policy_private_key.pem


source ../maestro/create_policy.sh

create_policy "$MAESTRO_URL" "policy_private_key.pem" "$BASE64_SERIALIZED_POLICY"

rm policy_private_key.pem
