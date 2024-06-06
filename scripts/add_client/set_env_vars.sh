echo "#### Setting env vars ####"

##### env-specific variables, make sure that they are set correctly ####

AWS_REGION="us-west-2"
MAESTRO_URL="https://maestro.preprod.boltforte.io"

## dev env
#ENV_PREFIX="dev"
#AWS_ACCOUNT="267505102317"
#GATEWAY_URL="https://api.wallet-dev.forte.io"
#USER_POOL_ID="us-west-2_I0D1f1wrO"
#AWS_PROFILE="aa-wallet-dev"
#PRIVATE_KEY_ARN="arn:aws:secretsmanager:us-west-2:267505102317:secret:dev-mpc-compliance-private-key-DTIGFZ"


#staging env
ENV_PREFIX="staging"
AWS_ACCOUNT="675635739063"
GATEWAY_URL="https://api.wallet-staging.forte.io"
USER_POOL_ID="us-west-2_W7ITGjdbI"
AWS_PROFILE="aa-wallet-staging"
PRIVATE_KEY_ARN="arn:aws:secretsmanager:us-west-2:675635739063:secret:staging-mpc-compliance-private-key-ONaP9w"


echo "We will use these vars:"
echo "ENV_PREFIX:      $ENV_PREFIX"
echo "MAESTRO_URL:     $MAESTRO_URL"
echo "AWS_ACCOUNT:     $AWS_ACCOUNT"
echo "AWS_REGION:      $AWS_REGION"
echo "GATEWAY_URL:     $GATEWAY_URL"
echo "USER_POOL_ID:    $USER_POOL_ID"
echo "AWS_PROFILE:     $AWS_PROFILE"
echo "PRIVATE_KEY_ARN: $PRIVATE_KEY_ARN"
echo ""