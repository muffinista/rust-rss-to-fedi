#!/usr/bin/env bash

if [ -z "$DATABASE_URL" ]; then
  echo "Please set DATABASE_URL!"
  exit
fi


rustup component add llvm-tools-preview
export RUSTFLAGS="-Cinstrument-coverage"
export LLVM_PROFILE_FILE="profile/rustypub-%p-%m.profraw"

scripts/test

grcov profile/ -s . --binary-path ./target/debug/ -t html --branch --ignore-not-existing -o ./target/debug/coverage/


# https://vladfilippov.com/rust-code-coverage-tools/