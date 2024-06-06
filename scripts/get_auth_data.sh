echo "#### AUTHENTICATION ####"

REGION="us-west-2"

echo "Obtaining cognito and api gateway information..."
POOLS=$(aws --region $REGION cognito-idp list-user-pools --max-results=60)
POOL_ID=$(echo $POOLS | jq -r ".UserPools[] | select(.Name==\"$1-mpc-user-pool\") | .Id")

POOL=$(aws --region $REGION cognito-idp describe-user-pool --user-pool-id $POOL_ID)
DOMAIN=$(echo $POOL | jq -r ".UserPool.Domain")
OAUTH2_URL="https://$DOMAIN.auth.us-west-2.amazoncognito.com"
echo "COGNITO URL: $OAUTH2_URL"

CLIENTS=$(aws --region $REGION cognito-idp list-user-pool-clients --user-pool-id $POOL_ID)
CLIENT_ID=$(echo $CLIENTS | jq -r ".UserPoolClients[] | select(.ClientName==\"dev-access\") | .ClientId")

CLIENT=$(aws --region $REGION cognito-idp describe-user-pool-client --user-pool-id $POOL_ID --client-id $CLIENT_ID)
CLIENT_SECRET=$(echo $CLIENT | jq -r ".UserPoolClient.ClientSecret")

AUTH=$(echo -n "$CLIENT_ID:$CLIENT_SECRET" | base64 | tr -d '\n')
echo "Client Id: $CLIENT_ID"
echo "Auth token: $AUTH"

APIS=$(aws --region $REGION apigateway get-rest-apis)
API_ID=$(echo $APIS | jq -r ".items[] | select(.name==\"$1-mpc_gateway\") | .id")

APIG_URL="https://$API_ID.execute-api.us-west-2.amazonaws.com/$1-mpc_gateway"
echo "API GATEWAY URL: $APIG_URL"

