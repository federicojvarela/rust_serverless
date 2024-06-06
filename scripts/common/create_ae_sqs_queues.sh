#!/bin/bash


ENV_PREFIX=$1
CLIENT_ID=$2
AWS_REGION="us-west-2"

# Dev account
export AWS_ACCOUNT="267505102317"

#https://awscli.amazonaws.com/v2/documentation/api/latest/reference/sqs/create-queue.html
echo "#### Creating SQS request queue DLQ ####"
CREATE_SQS_REQUEST_QUEUE_DLQ_RESULT=$(aws sqs create-queue --queue-name "${ENV_PREFIX}-approver-request-queue-dlq" --region "$AWS_REGION" --attributes "{\"SqsManagedSseEnabled\": \"true\"}")
SQS_REQUEST_QUEUE_DLQ_URL=$(echo "$CREATE_SQS_REQUEST_QUEUE_DLQ_RESULT" | jq -r .QueueUrl)
SQS_REQUEST_QUEUE_DLQ_ARN="arn:aws:sqs:${AWS_REGION}:${AWS_ACCOUNT}:${ENV_PREFIX}-approver-request-queue-dlq"

echo "#### Creating SQS request queue ####"
CREATE_SQS_REQUEST_QUEUE_RESULT=$(aws sqs create-queue --queue-name "${ENV_PREFIX}-approver-request-queue" --region "$AWS_REGION" --attributes "{\"RedrivePolicy\":\"{\\\"deadLetterTargetArn\\\":\\\"$SQS_REQUEST_QUEUE_DLQ_ARN\\\",\\\"maxReceiveCount\\\":\\\"3\\\"}\",\"SqsManagedSseEnabled\":\"true\"}")
SQS_REQUEST_QUEUE_URL=$(echo "$CREATE_SQS_REQUEST_QUEUE_RESULT" | jq -r .QueueUrl)
SQS_REQUEST_QUEUE_ARN="arn:aws:sqs:${AWS_REGION}:${AWS_ACCOUNT}:${ENV_PREFIX}-approver-request-queue"

echo "#### Creating SQS response queue DLQ ####"
CREATE_SQS_RESPONSE_QUEUE_DLQ_RESULT=$(aws sqs create-queue --queue-name "${ENV_PREFIX}-approver-response-queue-dlq" --region "$AWS_REGION" --attributes "{\"SqsManagedSseEnabled\": \"true\"}")
SQS_RESPONSE_QUEUE_DLQ_URL=$(echo "$CREATE_SQS_RESPONSE_QUEUE_DLQ_RESULT" | jq -r .QueueUrl)
SQS_RESPONSE_QUEUE_DLQ_ARN="arn:aws:sqs:${AWS_REGION}:${AWS_ACCOUNT}:${ENV_PREFIX}-approver-response-queue-dlq"

echo "#### Creating SQS response queue ####"
CREATE_SQS_RESPONSE_QUEUE_RESULT=$(aws sqs create-queue --queue-name "${ENV_PREFIX}-approver-response-queue" --region "$AWS_REGION" --attributes "{\"RedrivePolicy\":\"{\\\"deadLetterTargetArn\\\":\\\"$SQS_RESPONSE_QUEUE_DLQ_ARN\\\",\\\"maxReceiveCount\\\":\\\"3\\\"}\",\"SqsManagedSseEnabled\":\"true\"}")
SQS_RESPONSE_QUEUE_URL=$(echo "$CREATE_SQS_RESPONSE_QUEUE_RESULT" | jq -r .QueueUrl)
SQS_RESPONSE_QUEUE_ARN="arn:aws:sqs:${AWS_REGION}:${AWS_ACCOUNT}:${ENV_PREFIX}-approver-response-queue"

echo "#### Queues Created Successfully"
echo "SQS_REQUEST_QUEUE_URL:  $SQS_REQUEST_QUEUE_URL"
echo "SQS_REQUEST_QUEUE_ARN:  $SQS_REQUEST_QUEUE_ARN"
echo "SQS_REQUEST_QUEUE_DLQ_URL:  $SQS_REQUEST_QUEUE_DLQ_URL"
echo "SQS_REQUEST_QUEUE_DLQ_ARN:  $SQS_REQUEST_QUEUE_DLQ_ARN"
echo "SQS_RESPONSE_QUEUE_URL: $SQS_RESPONSE_QUEUE_URL"
echo "SQS_RESPONSE_QUEUE_ARN: $SQS_RESPONSE_QUEUE_ARN"
echo "SQS_RESPONSE_QUEUE_DLQ_URL: $SQS_RESPONSE_QUEUE_DLQ_URL"
echo "SQS_RESPONSE_QUEUE_DLQ_ARN: $SQS_RESPONSE_QUEUE_DLQ_ARN"
