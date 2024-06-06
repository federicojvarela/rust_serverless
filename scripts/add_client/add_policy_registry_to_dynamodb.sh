#!/bin/bash

ENV_PREFIX=$1
CLIENT_ID=$2
CHAIN_ID=$3
ADDRESS=$4
POLICY=$5
AWS_REGION="$6"

if [ -z "$1" ]; then
  echo "Example: ./add_policy_registry_to_dynamodb.sh dev TestClient 11155111 0x2Ce2679B7eE5c08CE151F1fC6AaEC0c9da20A74D PolicyC us-west-2"
  exit 1
fi

#https://awscli.amazonaws.com/v2/documentation/api/latest/reference/dynamodb/put-item.html

# Check if DEFAULT policy exists
DEFAULT_POLICY_EXISTS=$(aws dynamodb get-item --table-name "${ENV_PREFIX}-address_policy_registry" --key "{\"pk\":{\"S\":\"CLIENT#$CLIENT_ID#CHAIN$CHAIN_ID\"},\"sk\":{\"S\":\"ADDRESS#DEFAULT\"}}" --region "$AWS_REGION" --query "Item" --output text)


if [ "$DEFAULT_POLICY_EXISTS" = "None" ]; then
  echo "#### DEFAULT policy does not exist, adding it"
  DATE_STRING=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
  aws dynamodb put-item \
  --table-name "${ENV_PREFIX}-address_policy_registry" \
  --item "{\"pk\":{\"S\":\"CLIENT#$CLIENT_ID#CHAIN_ID#$CHAIN_ID\"},\"sk\":{\"S\":\"ADDRESS#DEFAULT\"},\"policy\":{\"S\":\"${ENV_PREFIX}_DefaultTenantSoloApproval\"},\"created_at\":{\"S\":\"$DATE_STRING\"},\"client_id\":{\"S\":\"$CLIENT_ID\"},\"chain_id\":{\"N\":\"$CHAIN_ID\"}}" \
  --region "$AWS_REGION"
  echo "#### DEFAULT policy added to DynamoDB successfully"
fi

# Check if the new policy item already exists
NEW_POLICY_EXISTS=$(aws dynamodb get-item --table-name "${ENV_PREFIX}-address_policy_registry" --key "{\"pk\":{\"S\":\"CLIENT#$CLIENT_ID#CHAIN$CHAIN_ID\"},\"sk\":{\"S\":\"ADDRESS#$ADDRESS\"}}" --region "$AWS_REGION" --query "Item" --output text)


if [ "$NEW_POLICY_EXISTS" = "None" ]; then
  echo "#### New policy does not exist, adding it"
  DATE_STRING=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
  aws dynamodb put-item \
  --table-name "${ENV_PREFIX}-address_policy_registry" \
  --item "{\"pk\":{\"S\":\"CLIENT#$CLIENT_ID#CHAIN_ID#$CHAIN_ID\"},\"sk\":{\"S\":\"ADDRESS#$ADDRESS\"},\"policy\":{\"S\":\"$POLICY\"},\"created_at\":{\"S\":\"$DATE_STRING\"},\"client_id\":{\"S\":\"$CLIENT_ID\"},\"chain_id\":{\"N\":\"$CHAIN_ID\"},\"address\":{\"S\":\"$ADDRESS\"}}" \
  --region "$AWS_REGION"
  echo "#### New policy added to DynamoDB successfully"
else
  echo "#### Policy for this PK and SK already exists, skipping put-item"
fi
