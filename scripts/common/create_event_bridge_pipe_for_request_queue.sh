#!/bin/bash

ENV_PREFIX=$1
CLIENT_ID=$2
SQS_REQUEST_QUEUE_ARN=$3
AWS_REGION="us-west-2"

# Dev account
export AWS_ACCOUNT="267505102317"

APPROVER_LAMBDA_ARN="arn:aws:lambda:${AWS_REGION}:${AWS_ACCOUNT}:function:${ENV_PREFIX}-mpc-default-client-approver"

# Create IAM policy for the pipe
#https://awscli.amazonaws.com/v2/documentation/api/latest/reference/iam/create-policy.html
echo "#### Creating pipe iam policy ####"

# Write template json to file, unlike response queue pipe, the request pipe needs access to a lambda, not a step function
echo -n '{"Version":"2012-10-17","Statement":[{"Effect":"Allow","Action":["sqs:ReceiveMessage","sqs:DeleteMessage","sqs:GetQueueAttributes"],"Resource":["${sqs_request_queue_arn}"]},{"Effect":"Allow","Action":["lambda:InvokeFunction"],"Resource":["${approver_lambda_arn}"]}]}' > pipe-iam-policy-doc-template.json

# populate the template file `pipe-iam-policy-doc-template.json` with correct values
POLICY_DOC_JSON=$(sed -e "s/\${sqs_request_queue_arn}/${SQS_REQUEST_QUEUE_ARN}/" -e "s/\${approver_lambda_arn}/${APPROVER_LAMBDA_ARN}/" pipe-iam-policy-doc-template.json)
# write it to a temp file;
# note: it will contain a newline char at the end that will cause parse error if left there
TMP_CLIENT_FILE="$CLIENT_ID.policy.txt"
echo -n "$POLICY_DOC_JSON" > "$TMP_CLIENT_FILE"

aws iam create-policy \
--policy-name "${ENV_PREFIX}-approver-request-pipe-policy" \
--policy-document "file://$TMP_CLIENT_FILE" \
--region "$AWS_REGION"
## remove the temp files, we don't need them anymore
rm "$TMP_CLIENT_FILE"
rm pipe-iam-policy-doc-template.json

# Create IAM role for pipe, and link it to the policy we just made

#https://awscli.amazonaws.com/v2/documentation/api/latest/reference/iam/create-role.html
echo "#### Creating pipe iam role ####"

# Write role json
echo -n '{"Version":"2012-10-17","Statement":[{"Action":"sts:AssumeRole","Effect":"Allow","Principal":{"Service":"pipes.amazonaws.com"}}]}' > pipe-iam-role-policy.json

aws iam create-role \
--role-name "${ENV_PREFIX}-approver-request-pipe-role" \
--assume-role-policy-document file://pipe-iam-role-policy.json \
--region "$AWS_REGION"

#https://awscli.amazonaws.com/v2/documentation/api/latest/reference/iam/attach-role-policy.html
echo "#### Attaching pipe iam policy to iam role ####"
aws iam attach-role-policy \
--role-name "${ENV_PREFIX}-approver-request-pipe-role" \
--policy-arn "arn:aws:iam::${AWS_ACCOUNT}:policy/${ENV_PREFIX}-approver-request-pipe-policy" \
--region "$AWS_REGION"

# Remove file
rm pipe-iam-role-policy.json

# Create the pipe now with the role we just made

#https://awscli.amazonaws.com/v2/documentation/api/latest/reference/pipes/create-pipe.html   
echo "#### Creating EB request pipe: ${ENV_PREFIX}-approver-request-pipe ####"

aws pipes create-pipe \
--name "${ENV_PREFIX}-approver-request-pipe" \
--role-arn "arn:aws:iam::${AWS_ACCOUNT}:role/${ENV_PREFIX}-approver-request-pipe-role" \
--source "$SQS_REQUEST_QUEUE_ARN" \
--target "$APPROVER_LAMBDA_ARN" \
--region "$AWS_REGION"

echo "#### EB pipe added successfully"
