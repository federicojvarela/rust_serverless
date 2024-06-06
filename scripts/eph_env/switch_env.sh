#!/bin/bash

SCRIPT_NAME=`basename "$0"`

set -u

if [ -z "$1" ]
  then
    echo "Error: No environment prefix specified as the first argument"
    echo "Usage: ${SCRIPT_NAME} PREFIX_ENV"
    exit 1
fi

source ../aws_config.sh

cd ../../infrastructure/terraform

WORKSPACE_EXISTS=`terraform workspace list | grep $1 | tr -d '* '`

if [ "$WORKSPACE_EXISTS" = "$1" ]; then
    terraform workspace select $1 
    terraform workspace list
else
    echo "ERROR: terraform workspace does not exist ($1)"
fi