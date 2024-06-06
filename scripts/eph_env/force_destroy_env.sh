#!/bin/bash

SCRIPT_NAME=$(basename "$0")

# List of environments we cant delete
UNDELETABLE_ENVIRONMENTS=("dev" "qa" "staging" "loadtest" "prod")
REGION="us-west-2"

while [[ $# -gt 0 ]]; do
    key="$1"
    case $key in
        -p | --prefix)
            prefix="$2"
            shift
            shift
            ;;
        -r | --region)
            REGION="$2"
            shift
            shift
            ;;
        -h | --help)
            echo "Usage: $SCRIPT_NAME [OPTIONS]"
            echo "Options:"
            echo "-p, --prefix    set prefix. i.e: ./$SCRIPT_NAME -p wall-42"
            echo "-r, --region    [Optional] set region. i.e: ./$SCRIPT_NAME -p wall-42 -r us-west-1. Default value: us-west-2"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            echo "Use -h or --help for usage information."
            exit 1
            ;;
    esac
done

# Check if we can delete the eph environment
if [[ " ${UNDELETABLE_ENVIRONMENTS[*]} " == *" $prefix "* ]]; then
    echo "error: an env with the prefix $prefix can't be deleted"
    exit 1
fi

##
# General
##

function main() {
    read -p "Are you sure? This will erase ALL resources in ephemeral environments with prefix '$prefix'. Please make sure the environment is yours and you can delete it. [yY/nN] " -n 1 -r
  if [[ $REPLY =~ ^[Yy]$ ]]
  then
      echo ""
      echo "Service: Secrets Manager"
      secrets_manager_delete_secrets
      echo "Service: S3"
      s3_delete_objects
      echo "Service: Lambda"
      lambda_delete_functions
      echo "Service: SQS"
      sqs_delete_queues
      echo "Service: Step Function"
      step_functions_delete_step_machines
      echo "Service: DynamoDB"
      dynamodb_delete_dynamo_objects
      echo "Service: Api Gateway"
      apigateway_delete_rest_apis
      echo "Service: EventBridge"
      eventbridge_delete_pipes
      eventbridge_delete_rules
      echo "Service: Cognito"
      cognito_delete_domain
      cognito_delete_user_pools
      echo "Service: IAM"
      iam_dettach_policy_from_role
      iam_delete_policies
      iam_delete_roles
      echo "Service: EventBridge"
      destroy_event_bridge_rules
      echo "Service: CloudWatch"
      cloudwatch_delete_loggroups
  fi
}

##
# Service: Secrets Manager
##
function secrets_manager_delete_secrets() {
    # List all Secrets Manager secrets
    secrets=$(aws secretsmanager list-secrets --region $REGION --query "SecretList[?starts_with(\"Name\", '$prefix')].Name" --output json | jq -r '.[]')

    # Loop through the secrets and delete them
    for secret in $secrets; do
        echo "Deleting Secrets Manager secret: $secret"
        aws secretsmanager delete-secret --region $REGION --secret-id "$secret"
    done
}

##
# Service: S3
##

function s3_delete_objects() {
    BUCKET_NAME="mpc-ephemeral-envs"

    # List all objects in the S3 bucket
    objects=$(aws s3 ls "s3://$BUCKET_NAME/" --recursive --region "$REGION")

    # Loop through the objects and delete them
    while read -r object; do
        object_key=$(echo "$object" | awk '{print $4}')
        if [[ "$object_key" == "$prefix"* ]]; then
            echo "Deleting S3 object: $object_key"
            aws s3 rm "s3://$BUCKET_NAME/$object_key" --region "$REGION"
        fi
    done <<< "$objects"
}

##
# Service: API Gateway
##

function apigateway_delete_rest_apis() {
    restapis=$(aws apigateway --region us-west-2 get-rest-apis | jq -r '.items[] | select(.name | startswith("'$prefix'")) | .id')

    for restapi in $restapis; do
        echo "Deleting Rest API: $restapi"
        aws apigateway --region $REGION delete-rest-api --rest-api-id $restapi
    done
}

##
# Service: Step Function
##

function step_functions_delete_step_machines() {
    # List all state machines
    state_machines=$(aws stepfunctions list-state-machines --region $REGION --query 'stateMachines[*].[name,stateMachineArn]' --output text)

    # Iterate over all state machines
    while read -r name arn; do
        if [[ $name == *"$prefix"* ]]; then
            echo "Deleting $name..."
            aws stepfunctions delete-state-machine --state-machine-arn $arn --region $REGION
        fi
    done <<< "$state_machines"
}

##
# Service: DynamoDB
##

function dynamodb_delete_dynamo_objects() {
    # List all DynamoDB tables
    tables=$(aws dynamodb list-tables --region $REGION --output json | jq -r '.TableNames[] | select(. | startswith("'$prefix'"))')

    # Loop through the tables and delete them
    for table in $tables; do
       echo "Deleting DynamoDB table: $table"
       aws dynamodb delete-table --table-name "$table" --region $REGION
    done
}

##
# Service: Cognito
##

function cognito_delete_user_pools() {
    # List all Cognito User Pools
    user_pools=$(aws cognito-idp --region $REGION list-user-pools --max-results 60 --output json | jq -r '.UserPools[] | select(.Name | startswith("'$prefix'")) | .Id')

    # Loop through the User Pools and delete them
    for user_pool_id in $user_pools; do
        echo "Deleting Cognito User Pool: $user_pool_id"
        aws cognito-idp --region $REGION delete-user-pool --user-pool-id "$user_pool_id"
    done
}

function cognito_delete_domain() {
    # Get the User Pool ID of the pool with the name containing "$prefix"
    user_pool_id=$(aws cognito-idp --region $REGION list-user-pools --max-results 60 | jq -r '.UserPools[] | select(.Name | startswith("'$prefix'")) | .Id')

    # Check if User Pool ID was found
    if [ -z "$user_pool_id" ]; then
        echo "No User Pool found with name containing '$prefix'"
    fi

    if [ -n "$user_pool_id" ]; then
        echo "User Pool ID: $user_pool_id"

        # Get the domain name
        domain=$(aws cognito-idp --region $REGION describe-user-pool --user-pool-id $user_pool_id | jq -r '.UserPool.Domain')

        # Check if Domain was found
        if [ -z "$domain" ]; then
            echo "No Domain found for User Pool ID: $user_pool_id"
        else
            echo "Domain: $domain"

            # Delete the domain
            aws cognito-idp --region $REGION delete-user-pool-domain  --domain $domain --user-pool-id $user_pool_id

            # Delete the User Pool
            aws cognito-idp --region $REGION delete-user-pool --user-pool-id $user_pool_id
        fi
    fi
}

##
# Service: CloudWatch
##

function cloudwatch_delete_loggroups() {
    # we need to search for log groups with the $prefix not only as a prefrix because there are some loggroups that are created
    # with the following prefix `/aws/events/` and `/aws/lambda/` but they need to be deleted because they belong to the environments  
    # which is about to get deleted.
    log_groups=$(aws --region $REGION logs describe-log-groups --log-group-name-pattern $prefix | jq -r '.logGroups[] | .logGroupName')

    for log_group in $log_groups; do
        echo "Deleting log group: $log_group"
        aws --region $REGION logs delete-log-group --log-group-name $log_group
    done
}

##
# Service: EventBridge
##

function eventbridge_delete_pipes() {
    pipes=$(aws --region $REGION pipes list-pipes --name-prefix $prefix | jq -r '.Pipes[] | .Name')

    for pipe in $pipes; do
        echo "Deleting EventBridge pipe: $pipe"
        aws --region $REGION pipes delete-pipe --name $pipe
    done
}

function eventbridge_delete_rules() {
    rules=$(aws --region $REGION events list-rules --name-prefix $prefix | jq -r '.Rules[] | .Name')

    for rule in $rules; do
        echo "Deleting EventBridge rule: $rule"

        echo "Getting EventBridge rule targest for $rule"
        targets=$(aws --region $REGION events list-targets-by-rule --rule $rule | jq -r '.Targets[] | .Id')

        for target in $targets; do
          echo "Deleting target $target for rule: $rule"
          aws --region $REGION events remove-targets --rule $rule --ids $target
        done

        aws --region $REGION events delete-rule --name $rule
    done
}

##
# Service: Lambda
##

function lambda_delete_functions() {
    lambdas=$(aws lambda --region $REGION list-functions | jq -r '.Functions[] | select(.FunctionName | startswith("'$prefix'")) | .FunctionArn')

    for lambda in $lambdas; do
        echo "Deleting lambda: $lambda"
        aws lambda --region $REGION delete-function --function-name $lambda
    done
}

##
# Service: SQS
##

function sqs_delete_queues() {
    queues=$(aws sqs --region $REGION list-queues --queue-name-prefix $prefix | jq -r '.QueueUrls | .[]')

    for queue in $queues; do
        echo "Deleting queue: $queue"
        aws sqs --region $REGION delete-queue --queue-url $queue
    done
}

##
# Service: IAM
##

function iam_dettach_policy_from_role() {
    # List all IAM roles containing the $prefix
    roles=$(aws iam list-roles --region $REGION --output json --query "Roles[?starts_with(\"RoleName\", '$prefix')]" | jq -r '.[] | .RoleName')

    # Loop through the roles
    for role in $roles; do
        echo "Detaching IAM policy from role: $role"

        # List all attached MANAGED policies for the role
        attached_policies=$(aws iam list-attached-role-policies --region $REGION --role-name "$role" --output json | jq -r '.AttachedPolicies[] | select(.PolicyName | startswith("'$prefix'")) | .PolicyArn')

        # Detach policies from the role
        for policy_arn in $attached_policies; do
            aws iam detach-role-policy --role-name "$role" --policy-arn "$policy_arn" --region $REGION
            echo "Detached policy: $policy_arn"
        done

        # List all attached INLINE policies for the role
        attached_inline_policies=$(aws iam list-role-policies --region $REGION --role-name "$role" --output json | jq -r '.PolicyNames[]')

        # Delete policies from the role
        for policy_name in $attached_inline_policies; do
            aws iam delete-role-policy --role-name "$role" --policy-name "$policy_name" --region $REGION
            echo "Deleting inline policy: $policy_name"
        done
    done

}

function iam_delete_policies() {
    # List all IAM policies
    policies=$(aws iam list-policies --region $REGION --output json --query "Policies[?starts_with(\"PolicyName\", '$prefix')]" | jq -r '.[] | .PolicyName')

    # Loop through the policies and delete them
    for policy in $policies; do
        echo "Deleting IAM policy: $policy"
        aws iam delete-policy --policy-arn --region $REGION "$(aws iam list-policies --region $REGION --scope Local --query "Policies[?PolicyName=='$policy'].Arn" --output text)"
    done
}

function iam_delete_roles() {
    # List all IAM roles
    roles=$(aws iam list-roles --region $REGION --output json --query "Roles[?starts_with(\"RoleName\", '$prefix')]" | jq -r '.[] | .RoleName')

    for role in $roles; do
        echo "Deleting IAM role: $role"
        aws iam delete-role --role-name "$role" --region $REGION
    done
}

function destroy_event_bridge_rules() {
    echo "Destroying all EventBridge rules for prefix: $prefix"

    # List all rules for the event bus that start with the prefix
    rules=$(aws events list-rules --name-prefix "$prefix" --region $REGION --output json | jq -r '.Rules[].Name')

    # Loop through the rules and delete them
    for rule in $rules; do
        echo "Deleting EventBridge rule: $rule"
        aws events delete-rule --name "$rule" --region $REGION
    done
}

##
# Main execution
##
main
