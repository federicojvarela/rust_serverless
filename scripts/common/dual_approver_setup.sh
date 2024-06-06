#!/bin/bash

while [[ $# -gt 0 ]]; do
    key="$1"
    case $key in
        -e | --env)
            ENV_PREFIX="$2"
            shift
            shift
            ;;
        -c | --client_id)
            CLIENT_ID="$2"
            shift
            shift
            ;;
        -h | --help)
            echo "Usage: $SCRIPT_NAME [OPTIONS]"
            echo "Options:"
            echo "-e, --env    set env. i.e: ./$SCRIPT_NAME -e dev"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            echo "Use -h or --help for usage information."
            exit 1
            ;;
    esac
done

if [ "$ENV_PREFIX" == "dev" ]; then
  echo "error: environment: '$ENV_PREFIX' isn't supported."
  exit 1
elif [ "$ENV_PREFIX" == "staging" ]; then
  echo "error: environment: '$ENV_PREFIX' isn't supported."
  exit 1
fi

AE_NAME="${CLIENT_ID}_ae"
AWS_REGION="us-west-2"

echo "#### Starting MultiApprover MS2 Setup for: $ENV_PREFIX"

# Setup the queues
# source it so we can reference the variables in scope of this wrapper script
source create_ae_sqs_queues.sh "$ENV_PREFIX" "$CLIENT_ID" "$AWS_REGION"

# Update dynamo table
echo "#### Adding queues to authorizing_entities table in dynamo"
./add_ae_record_to_dynamo.sh "$ENV_PREFIX" "$AE_NAME" "$SQS_REQUEST_QUEUE_URL" "$SQS_REQUEST_QUEUE_ARN" "$AWS_REGION"

# Create event bridge pipe for the response queue
echo "#### Creating event bridge pipe for the response queue"
./create_event_bridge_pipe_for_response_queue.sh "$ENV_PREFIX" "$CLIENT_ID" "$SQS_RESPONSE_QUEUE_ARN" "$AWS_REGION"

# Deploy new approver
echo "#### Deploying Client Approver Lambda"

cd ../auto_approver
./deploy_approver.sh "$ENV_PREFIX" "$CLIENT_ID" "$SQS_RESPONSE_QUEUE_URL"
cd ../common

# Create event bridge pipe for the request queue (depends on lambda being deployed)
./create_event_bridge_pipe_for_request_queue.sh "$ENV_PREFIX" "$CLIENT_ID" "$SQS_REQUEST_QUEUE_ARN" "$AWS_REGION"

# Add an IAM Policy so that the Send Transaction to Approvers workflow can post messages to the new request-queue
./add_sqs_access_to_send_transaction_to_approvers_sm.sh "$ENV_PREFIX" "$CLIENT_ID" "$SQS_REQUEST_QUEUE_ARN"

echo "#### MultiApprover MS2 Setup Complete for: $ENV_PREFIX"
