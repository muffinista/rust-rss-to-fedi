#!/usr/bin/env bash

if [ -z "$DATABASE_URL" ]; then
  echo "Please set DATABASE_URL!"
  exit
fi


rustup component add llvm-tools-preview
export RUSTFLAGS="-Cinstrument-coverage"
export LLVM_PROFILE_FILE="profile/rustypub-%p-%m.profraw"

cargo +nightly test


rm -rf target/

scripts/test

#grcov ../rust-activitypub/profile/ -s . --binary-path ../rust-activitypub/target/debug/ -t html --llvm --branch --ignore-not-existing -o ../rust-activitypub/target/debug/coverage/
grcov . --binary-path ./target/debug/deps/ -s . -t html --branch --ignore-not-existing --ignore '../*' --ignore "/*" -o target/coverage/html

# https://vladfilippov.com/rust-code-coverage-tools/