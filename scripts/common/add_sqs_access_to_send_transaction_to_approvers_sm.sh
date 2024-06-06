#!/bin/bash

ENV_PREFIX=$1
CLIENT_ID=$2
SQS_REQUEST_QUEUE_ARN=$3

echo "#### Creating SQS access policy (request-queue) for the Send Transaction to Approvers workflow  ####"

echo -n '{"Version":"2012-10-17","Statement":[{"Effect":"Allow","Action":["sqs:SendMessage"],"Resource":["${sqs_request_queue_arn}"]}]}' > request-queue-access-iam-policy-doc-template.json

# populate the template file `request-queue-access-iam-policy-doc-template.json` with correct values
POLICY_DOC_JSON=$(sed -e "s/\${sqs_request_queue_arn}/${SQS_REQUEST_QUEUE_ARN}/" request-queue-access-iam-policy-doc-template.json)
# write it to a temp file;
# note: it will contain a newline char at the end that will cause parse error if left there
TMP_CLIENT_FILE="$CLIENT_ID.policy.txt"
echo -n "$POLICY_DOC_JSON" > "$TMP_CLIENT_FILE"

RESPONSE=$(aws iam create-policy \
--policy-name "${ENV_PREFIX}-request-queue-access-iam-policy" \
--policy-document "file://$TMP_CLIENT_FILE" \
--region "$AWS_REGION")

POLICY_ARN=$(echo "$RESPONSE" | jq -r ".Policy.Arn")

## remove the temp files, we don't need them anymore
rm "$TMP_CLIENT_FILE"
rm request-queue-access-iam-policy-doc-template.json

echo "#### Attaching policy to iam role ####"
aws iam attach-role-policy \
--role-name "${ENV_PREFIX}-send_transaction_to_approvers" \
--policy-arn "$POLICY_ARN" \
--region "$AWS_REGION"

