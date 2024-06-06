#!/bin/bash

# If you are using placeholder path parameters that need to be resolved
# you will need to modify the "pathParameters" part of the script to
# call them out
# e.g.  
# 	\"pathParameters\": {
#		\"key_id\": \"0b0b0a84-a697-4518-988e-929303250352\"
#	},

ERROR_FOUND=0

if [ -z "$1" ]
  then
    echo "Error: No lambda name specified as the first argument"
    ERROR_FOUND=1
fi

if [ -z "$2" ]
  then
    echo "Error: No URL path specified as the second argument"
    ERROR_FOUND=1
fi

if [ $ERROR_FOUND -eq 1 ]
  then
    SCRIPT_NAME=`basename "$0"`
    echo "Usage: ${SCRIPT_NAME} NAME_OF_LAMBDA URL_PATH"
    exit 1
fi

cargo lambda invoke $1 --data-ascii "{
	\"resource\": \"/{proxy+}\",
	  \"path\": \"$2\",
	  \"httpMethod\": \"POST\",
	  \"headers\": {
		  \"Accept\": \"*/*\",
		  \"Accept-Encoding\": \"gzip, deflate\",
		  \"cache-control\": \"no-cache\",
		  \"CloudFront-Forwarded-Proto\": \"https\",
		  \"CloudFront-Is-Desktop-Viewer\": \"true\",
		  \"CloudFront-Is-Mobile-Viewer\": \"false\",
		  \"CloudFront-Is-SmartTV-Viewer\": \"false\",
		  \"CloudFront-Is-Tablet-Viewer\": \"false\",
		  \"CloudFront-Viewer-Country\": \"US\",
		  \"Content-Type\": \"application/json\",
		  \"headerName\": \"headerValue\",
		  \"Host\": \"gy415nuibc.execute-api.us-east-1.amazonaws.com\",
		  \"Postman-Token\": \"9f583ef0-ed83-4a38-aef3-eb9ce3f7a57f\",
		  \"User-Agent\": \"PostmanRuntime/2.4.5\",
		  \"Via\": \"1.1 d98420743a69852491bbdea73f7680bd.cloudfront.net (CloudFront)\",
		  \"X-Amz-Cf-Id\": \"pn-PWIJc6thYnZm5P0NMgOUglL1DYtl0gdeJky8tqsg8iS_sgsKD1A==\",
		  \"X-Forwarded-For\": \"54.240.196.186, 54.182.214.83\",
		  \"X-Forwarded-Port\": \"443\",
		  \"X-Forwarded-Proto\": \"https\"
    },
    \"multiValueHeaders\": {
        \"Accept\": [\"*/*\"],
        \"Accept-Encoding\": [\"gzip, deflate\"],
        \"cache-control\": [\"no-cache\"],
        \"CloudFront-Forwarded-Proto\": [\"https\"],
        \"CloudFront-Is-Desktop-Viewer\": [\"true\"],
        \"CloudFront-Is-Mobile-Viewer\": [\"false\"],
        \"CloudFront-Is-SmartTV-Viewer\": [\"false\"],
        \"CloudFront-Is-Tablet-Viewer\": [\"false\"],
        \"CloudFront-Viewer-Country\": [\"US\"],
        \"Content-Type\": [\"application/json\"],
        \"headerName\": [\"headerValue\"],
        \"Host\": [\"gy415nuibc.execute-api.us-east-1.amazonaws.com\"],
        \"Postman-Token\": [\"9f583ef0-ed83-4a38-aef3-eb9ce3f7a57f\"],
        \"User-Agent\": [\"PostmanRuntime/2.4.5\"],
        \"Via\": [\"1.1 d98420743a69852491bbdea73f7680bd.cloudfront.net (CloudFront)\"],
        \"X-Amz-Cf-Id\": [\"pn-PWIJc6thYnZm5P0NMgOUglL1DYtl0gdeJky8tqsg8iS_sgsKD1A==\"],
        \"X-Forwarded-For\": [\"54.240.196.186, 54.182.214.83\"],
        \"X-Forwarded-Port\": [\"443\"],
        \"X-Forwarded-Proto\": [\"https\"]
    },
	\"queryStringParameters\": {
    },
    \"multiValueQueryStringParameters\": {
    },
	\"pathParameters\": {
		\"key_id\": \"0b0b0a84-a697-4518-988e-929303250352\"
	},
	\"stageVariables\": {
		\"stageVariableName\": \"stageVariableValue\"
	},
	\"requestContext\": {
		\"accountId\": \"12345678912\",
		\"resourceId\": \"roq9wj\",
		\"path\": \"$2\",
		\"stage\": \"mpc_gateway\",
		\"domainName\": \"gy415nuibc.execute-api.us-east-2.amazonaws.com\",
		\"domainPrefix\": \"y0ne18dixk\",
		\"requestId\": \"deef4878-7910-11e6-8f14-25afc3e9ae33\",
		\"protocol\": \"HTTP/1.1\",
		\"identity\": {
			\"cognitoIdentityPoolId\": \"theCognitoIdentityPoolId\",
			\"accountId\": \"theAccountId\",
			\"cognitoIdentityId\": \"theCognitoIdentityId\",
			\"caller\": \"theCaller\",
            \"apiKey\": \"theApiKey\",
            \"apiKeyId\": \"theApiKeyId\",
            \"accessKey\": \"ANEXAMPLEOFACCESSKEY\",
			\"sourceIp\": \"192.168.196.186\",
			\"cognitoAuthenticationType\": \"theCognitoAuthenticationType\",
			\"cognitoAuthenticationProvider\": \"theCognitoAuthenticationProvider\",
			\"userArn\": \"theUserArn\",
			\"userAgent\": \"PostmanRuntime/2.4.5\",
			\"user\": \"theUser\"
		},
		\"authorizer\": {
			\"principalId\": \"admin\",
			\"clientId\": 1,
			\"clientName\": \"Exata\"
		},
		\"resourcePath\": \"/{proxy+}\",
		\"httpMethod\": \"POST\",
		\"requestTime\": \"15/May/2020:06:01:09 +0000\",
		\"requestTimeEpoch\": 1589522469693,
		\"apiId\": \"gy415nuibc\"
	},
	\"body\": \"{}\"
}"

