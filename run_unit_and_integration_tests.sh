#!/bin/bash

# Default values
VERBOSE="false"
FILTER=""
JOBS=""
REMOTE="false"
CARGO_LAMBDA_PID=""

run_tests() {
  if [ "$JOBS" == "" ]; then
    if ! cargo test "$FILTER" -- --test-threads 1 --nocapture
    then
        if [ "$CARGO_LAMBDA_PID" != "" ]
        then
          kill "$CARGO_LAMBDA_PID"
        fi
        echo "There are some errors running tests"
        exit 1
    fi
  else
    if ! cargo test -j "$JOBS" "$FILTER" -- --test-threads 1 --nocapture
    then
        if [ "$CARGO_LAMBDA_PID" != "" ]
        then
          kill "$CARGO_LAMBDA_PID"
        fi
        echo "There are some errors running tests"
        exit 1
    fi
  fi
}

while [[ $# -gt 0 ]]; do
    key="$1"
    case $key in
    -v | --verbose)
        VERBOSE="true"
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
    -r | --remote)
        REMOTE="true"
        shift
        ;;
    -h | --help)
        echo "Usage: ./run_unit_and_integration_tests.sh [OPTIONS]"
        echo "Options:"
        echo "-v, --verbose   set verbosity"
        echo "-f, --filter    filters tests to execute"
        echo "-j, --jobs      set number of jobs"
        echo "-r, --remote    does not setup local env"
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

# Load environment variables
if [ "$REMOTE" == "false" ]; then
  set -a
  source .env.test
  source .env.test.local
  set +a
fi

# Run crate tests
echo "Running the tests..."
echo "Running common tests..."
cd ./common || exit
run_tests
echo "Running model tests..."
cd ../model || exit
run_tests
echo "Running repositories tests..."
cd ../repositories || exit
run_tests

# Run main tests
cd ..

# Run cargo watch process
if [ "$REMOTE" == "false" ]; then
  set -a

  echo "Running lambda watch..."
  if [ "$VERBOSE" == "true" ]; then
      cargo lambda watch &
  else
      cargo lambda watch &>/dev/null &
  fi
  set +a
  CARGO_LAMBDA_PID=$!
fi

echo "Running main tests..."
run_tests

# Kill watch process
echo "Done"
if [ "$REMOTE" == "false" ]; then
  kill $CARGO_LAMBDA_PID
fi

exit 0