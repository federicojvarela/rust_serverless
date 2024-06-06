#!/bin/bash

echo "Obtaining cognito information for $1-mpc-user-pool..."
POOLS=$(aws cognito-idp list-user-pools --max-results=60)
echo $POOLS
POOL_ID=$(echo $POOLS | jq -r ".UserPools[] | select(.Name==\"$1-mpc-user-pool\") | .Id")

CLIENTS=$(aws cognito-idp list-user-pool-clients --user-pool-id $POOL_ID)
CLIENT_ID=$(echo $CLIENTS | jq -r ".UserPoolClients[] | select(.ClientName==\"dev-access\") | .ClientId")