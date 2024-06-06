#!/bin/bash

export AWS_PAGER=""

export ENV_PREFIX=$1
export APPROVER_SUFFIX=$2
export AWS_REGION=us-west-2

RESPONSE_QUEUE_URL=$3

export AWS_ACCOUNT="267505102317"
if [ "$ENV_PREFIX" == "dev" ]; then
  echo "error: ENV_PREFIX: '$ENV_PREFIX' isn't supported. "
  exit 1
elif [ "$ENV_PREFIX" == "staging" ]; then
  echo "error: ENV_PREFIX: '$ENV_PREFIX' isn't supported. "
  exit 1
fi

CURRENT_DIR=$(pwd)
cd ../..

cargo lambda build --release --output-format zip

cd $CURRENT_DIR

# Create the policies envs
LOGS_POLICY_NAME="${ENV_PREFIX}-client-approver-lambda-logs"
ARN_LOGS_POLICY="arn:aws:iam::$AWS_ACCOUNT:policy/${LOGS_POLICY_NAME}"

SECRETS_MANAGER_POLICY_NAME="${ENV_PREFIX}-client-approver-lambda-read-secrets"
ARN_SECRETS_MANAGER_POLICY="arn:aws:iam::$AWS_ACCOUNT:policy/${SECRETS_MANAGER_POLICY_NAME}"

SQS_POLICY_NAME="${ENV_PREFIX}-approver-lambda-sqs-permissions"
ARN_SQS_POLICY="arn:aws:iam::$AWS_ACCOUNT:policy/${SQS_POLICY_NAME}"

# Create the role envs
ROLE_NAME="${ENV_PREFIX}-mpc-default-client-approver"
ARN_ROLE_NAME="arn:aws:iam::$AWS_ACCOUNT:role/${ROLE_NAME}"

export LAMBDA_NAME="${ENV_PREFIX}-mpc-default-client-approver"

# Substitute the env vars in the policy json files
envsubst < logs_policy.json> logs.json
envsubst < secrets_manager_policy.json> secrets_manager.json
envsubst < sqs_policy.json> sqs.json

# Create the log groups
aws logs create-log-group --log-group-name "/aws/lambda/${LAMBDA_NAME}" --region "$AWS_REGION"

# Create the policies
aws  iam create-policy --policy-name "$LOGS_POLICY_NAME" --policy-document file://logs.json --region "$AWS_REGION"
rm logs.json

aws iam create-policy --policy-name "$SECRETS_MANAGER_POLICY_NAME" --policy-document file://secrets_manager.json --region "$AWS_REGION"
rm secrets_manager.json

aws iam create-policy --policy-name "$SQS_POLICY_NAME" --policy-document file://sqs.json --region "$AWS_REGION"
rm sqs.json

# Create the role
aws iam create-role --role-name "$ROLE_NAME" --assume-role-policy-document file://trust_policy.json --region "$AWS_REGION"

# Attach policies to role
aws iam attach-role-policy --role-name "$ROLE_NAME" --policy-arn "$ARN_SECRETS_MANAGER_POLICY" --region "$AWS_REGION"

aws iam attach-role-policy --role-name "$ROLE_NAME" --policy-arn "$ARN_LOGS_POLICY" --region "$AWS_REGION"

aws iam attach-role-policy --role-name "$ROLE_NAME" --policy-arn "$ARN_SQS_POLICY" --region "$AWS_REGION"

cd ../..

# Deploy local auto approver lambda to AWS
APPROVER_NAME="${APPROVER_SUFFIX}_ae"
APPROVER_PRIVATE_KEY_SECRET_NAME="$ENV_PREFIX-mpc-compliance-private-key"
aws lambda create-function --function-name "$LAMBDA_NAME" --timeout 20 --handler mpc_default_approver --zip-file fileb://target/lambda/mpc_default_approver/bootstrap.zip --role "$ARN_ROLE_NAME" \
--runtime provided.al2 --environment Variables="{auto_approver_result=approve,approver_name=$APPROVER_NAME,approver_private_key_secret_name=$APPROVER_PRIVATE_KEY_SECRET_NAME,response_queue_url=$RESPONSE_QUEUE_URL,send_sqs_message_wait_seconds=5}" --region "$AWS_REGION"

