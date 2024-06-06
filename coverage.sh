#!/bin/bash

### Stop containers if running and clear the volumes
docker compose --profile local_tests down

### Start the AWS services needed by the app
docker compose --profile local_tests up --build -d

### Configuration backups
RUSTFLAGS_BACKUP=$RUSTFLAGS
LLVM_PROFILE_FILE_BACKUP=$LLVM_PROFILE_FILE

### Install necessary tools
cargo install grcov
rustup component add llvm-tools-preview

### Configurations
CC_DIR=target/code_coverage
export LLVM_PROFILE_FILE="$CC_DIR/mpc-%p-%m.profraw"
export RUSTFLAGS="-C instrument-coverage"

### Clear out existing code coverage files and clear out compiled code so that it can be instrumented
cargo clean
rm -fr $CC_DIR

### Run tests
./run_unit_and_integration_tests.sh

### Run the code coverage report
echo "Generating report... this may take some time"

grcov . --llvm --binary-path target/debug -s . -t html \
  --ignore='tests*' --ignore='target*' \
  --excl-line="(#\[(derive|cfg_attr).*\]|(lambda_main!|http_lambda_main)!.*)" \
  --ignore="common/src/macros/*" \
  --ignore="common/src/config/*" \
  --ignore="common/src/mocks/*" \
  --ignore="common/src/aws_clients/*" \
  -o $CC_DIR

### Restore original configuration
export LLVM_PROFILE_FILE=$LLVM_PROFILE_FILE_BACKUP
export RUSTFLAGS=$RUSTFLAGS_BACKUP

### Open report
case "$(uname -s)" in
    Darwin*)
      open target/code_coverage/html/index.html
      exit 0;;
    Linux*)
      xdg-open target/code_coverage/html/index.html
      exit 0;;
    # The following are windows cases! Edit with a convenient tool
    # CYGWIN*)    ;;
    # MINGW*)     ;;
    *)
      echo "Unknown operating system kernel: $(uname -s)"
      exit 1
esac

