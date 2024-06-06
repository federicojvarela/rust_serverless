#!/bin/bash

if [ -z "$1" ]
  then
    echo "Error: No prefix_env specified as the first argument"
    exit 1
fi

source ../aws_config.sh

cd ../..

cargo lambda build --release --output-format zip

set -eu

UNIQUE_VALUE=`date +%s`

for BUILD_LAMBDA_FUNCTION_FILE_PATH in target/lambda/*/*.zip; do

    # BUILD_LAMBDA_FUNCTION_FILE_PATH: target/lambda/example_function_name/bootstrap.zip

    # FUNCTION_PATH: example_function_name/bootstrap.zip
    FUNCTION_PATH="${BUILD_LAMBDA_FUNCTION_FILE_PATH#*target/lambda/}"
 
    # FUNCTION_HANDLER: example_function_name
    FUNCTION_HANDLER="${FUNCTION_PATH%/*}"

    # FUNCTION_ZIP_FILE: bootstrap.zip
    FUNCTION_ZIP_FILE="${FUNCTION_PATH#*/}"


    OBJECT_KEY="$1/lambdas/${FUNCTION_HANDLER}-${UNIQUE_VALUE}/${FUNCTION_ZIP_FILE}"

    aws s3 cp ${BUILD_LAMBDA_FUNCTION_FILE_PATH} s3://mpc-ephemeral-envs/${OBJECT_KEY}

    OUTPUT+=("{\"function_name\":\"$1-${FUNCTION_HANDLER}\",\"handler_name\":\"${FUNCTION_HANDLER}\",\"bucket_id\":\"mpc-ephemeral-envs\",\"object_key\":\"${OBJECT_KEY}\"}")
done

$(IFS=, ; echo "[${OUTPUT[*]}]" > .ephemeral/$1-manifest.json)

aws s3 cp .ephemeral/$1-manifest.json s3://mpc-ephemeral-envs/$1/lambdas/manifest.json

echo "All lambdas generated and uploaded to S3"