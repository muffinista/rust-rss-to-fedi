#!/usr/bin/env bash

rustup component add llvm-tools-preview
export RUSTFLAGS="-Cinstrument-coverage"
export LLVM_PROFILE_FILE="profile/rustypub-%p-%m.profraw"
DOMAIN_NAME=foo.com DATABASE_URL=sqlite:database.sqlite cargo test
grcov . -s profile/ --binary-path ./target/debug/ -t html --branch --ignore-not-existing -o ./target/debug/coverage/
