#!/bin/bash

########################################################################
## This script helps you watch/debug AWS Lambda calls                 ##
##                                                                    ##
## How to use:                                                        ##
##   1. Run this script in a different terminal                       ##
##   2. In another terminal, use 'cargo test -- --test-threads 1'     ##
########################################################################

# Load environment variables
set -a
source .env.test
source .env.test.local

# Run cargo watch process
cargo lambda watch

set +a