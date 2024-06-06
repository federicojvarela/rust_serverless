#!/bin/bash

ENV_PREFIX=$1
AE_NAME=$2
SQS_REQUEST_QUEUE_URL=$3
SQS_REQUEST_QUEUE_ARN=$4
AWS_REGION="$5"

#https://awscli.amazonaws.com/v2/documentation/api/latest/reference/dynamodb/put-item.html
echo "#### Inserting url and arn of the new request queues into: ${ENV_PREFIX}-authorizing_entities ####"
DATE_STRING=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
aws dynamodb put-item \
--table-name "${ENV_PREFIX}-authorizing_entities" \
--item "{\"pk\": {\"S\": \"$AE_NAME\"}, \"request_sqs_url\": {\"S\": \"$SQS_REQUEST_QUEUE_URL\"}, \"request_sqs_arn\": {\"S\": \"$SQS_REQUEST_QUEUE_ARN\"}, \"date_time\": {\"S\": \"$DATE_STRING\"}}" \
--region "$AWS_REGION"

echo "#### Added to dynamo successfully"