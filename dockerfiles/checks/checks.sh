#!/bin/bash

if ! cargo fmt -- --check
then
  echo "There are some code style issues."
  echo "Run \"cargo fmt\" before commit."
  exit 1
fi

if ! cargo clippy --all-targets -- -D warnings
then
  echo "There are some clippy issues."
  exit 1
fi

# Ticket to investigate ignored error: https://forteio.atlassian.net/browse/WALL-157
if ! cargo audit --ignore RUSTSEC-2020-0071 --ignore RUSTSEC-2023-0052 --ignore RUSTSEC-2023-0052 --
then
  echo "cargo audit failing."
  echo "Run \"cargo audit\" before commit."
  exit 1
fi

exit 0
