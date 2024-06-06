EXAMPLE_FILE="../../e2e/env_files/.env.test.e2e.eph.example"
ENV_FILE="../../e2e/env_files/.env.test.e2e.$1.eph"
AUTH=$(echo "$AUTH" | tr -d '\n\r')

cp $EXAMPLE_FILE "$ENV_FILE"

echo "ENV_FILE -> $ENV_FILE"
echo "APIG_URL -> $APIG_URL"
echo "OAUTH2_URL -> $OAUTH2_URL"
echo "AUTH TOKEN -> $AUTH"
echo "CLIENT_ID -> $CLIENT_ID"
echo "ENV -> $1"

if [ -e "$ENV_FILE" ]; then
  sed -i '' "s|^ENV_URL=.*|ENV_URL=\"$APIG_URL\"|" "$ENV_FILE"
  sed -i '' "s|^AUTH_URL=.*|AUTH_URL=\"$OAUTH2_URL\"|" "$ENV_FILE"
  sed -i '' 's/^AUTHORIZATION_TOKEN=.*/AUTHORIZATION_TOKEN="Basic '"$AUTH"'"/' "$ENV_FILE"
  sed -i '' "s|^CLIENT_ID=.*|CLIENT_ID=\"$CLIENT_ID\"|" "$ENV_FILE"
  sed -i '' "s|^CUSTOM_APPROVER_NAME=.*|CUSTOM_APPROVER_NAME=\"${CLIENT_ID}_ae\"|" "$ENV_FILE"
  sed -i '' "s|^ENVIRONMENT=.*|ENVIRONMENT=\"$1\"|" "$ENV_FILE"
else
  echo "Error: .env.test.e2e.$1.eph file not found!"
fi