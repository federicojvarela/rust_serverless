#!/bin/bash
# This script creates a dummy for all the binaries included in Cargo.toml. This is used for caching
# the dependencies
LAMBDA_BINARIES=$(cat Cargo.toml | grep -A 2 "[[bin]]" | grep "path =" | cut -d= -f2 | sed 's/"//g')

for lambda in $LAMBDA_BINARIES
do
    mkdir -p $(dirname $lambda);
    touch $lambda;
    echo "fn main() {}" >> $lambda;
done
