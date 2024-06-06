#!/bin/bash

# Default values
ENV="dev"
CHAIN_NAME="ethereum"
FILTER=""
JOBS=""

while [[ $# -gt 0 ]]; do
    key="$1"
    case $key in
    -e | --env)
        ENV="$2"
        shift
        shift
        ;;
    -c | --chain)
        CHAIN_NAME="$2"
        shift
        shift
        ;;
    -f | --filter)
         FILTER="$2"
         shift
         shift
         ;;
    -j | --jobs)
        JOBS="$2"
        shift
        shift
        ;;
    -h | --help)
        echo "Usage: ./run_e2e_tests.sh [OPTIONS]"
        echo "Options:"
        echo "-e, --env       set environment"
        echo "-c, --chain     set chain ID"
        echo "-f, --filter    filters tests to execute"
        echo "-j, --jobs      set number of jobs"
        echo "-h, --help      display this help message"
        exit 0
        ;;
    *)
        echo "Unknown option: $1"
        echo "Use -h or --help for usage information."
        exit 1
        ;;
    esac
done

# Run tests
echo "Running e2e tests on ${ENV} -> ${CHAIN_NAME}..."
cd e2e || exit

if [ "$JOBS" == "" ]; then
  ENV="$ENV" CHAIN_NAME="$CHAIN_NAME" cargo test "$FILTER" -- --test-threads 1 --nocapture
else
  ENV="$ENV" CHAIN_NAME="$CHAIN_NAME" cargo test -j "$JOBS" "$FILTER" -- --test-threads 1 --nocapture
fi

exit_code=$?

if [ $exit_code -eq 0 ]; then
    echo "Tests passed successfully."
    exit 0
else
    echo "An error occurred with exit code $exit_code."
    exit 101
fi

cd ../
echo "Done"
