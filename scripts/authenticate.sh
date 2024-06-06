echo "#### AUTHENTICATION ####"

echo "Obtaining cognito and api gateway information..."
POOLS=$(aws cognito-idp list-user-pools --max-results=60)
POOL_ID=$(echo $POOLS | jq -r ".UserPools[] | select(.Name==\"$1-mpc-user-pool\") | .Id")

POOL=$(aws cognito-idp describe-user-pool --user-pool-id $POOL_ID)
DOMAIN=$(echo $POOL | jq -r ".UserPool.Domain")
OAUTH2_URL="https://$DOMAIN.auth.us-west-2.amazoncognito.com/oauth2/token"
echo
echo "COGNITO URL: $OAUTH2_URL"

CLIENTS=$(aws cognito-idp list-user-pool-clients --user-pool-id $POOL_ID)
CLIENT_ID=$(echo $CLIENTS | jq -r ".UserPoolClients[] | select(.ClientName==\"dev-access\") | .ClientId")

CLIENT=$(aws cognito-idp describe-user-pool-client --user-pool-id $POOL_ID --client-id $CLIENT_ID)
CLIENT_SECRET=$(echo $CLIENT | jq -r ".UserPoolClient.ClientSecret")

AUTH=$(echo -n "$CLIENT_ID:$CLIENT_SECRET" | base64 | tr -d '\n')

APIS=$(aws apigateway get-rest-apis)
API_ID=$(echo $APIS | jq -r ".items[] | select(.name==\"$1-mpc_gateway\") | .id")

APIG_URL="https://$API_ID.execute-api.us-west-2.amazonaws.com/$1-mpc_gateway"
echo "API GATEWAY URL: $APIG_URL"
echo

ACCESS_TOKEN=$(curl -s $OAUTH2_URL -H "Authorization: Basic $AUTH" -H "Content-Type: application/x-www-form-urlencoded" -d "grant_type=client_credentials&client_id=$CLIENT_ID" | jq -r ".access_token")