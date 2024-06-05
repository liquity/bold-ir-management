#!/bin/bash
cd ir_manager
cargo run --features export-api > candid.did
cd ..
cargo build --release --target wasm32-unknown-unknown --features export-api
# dfx canister status liquity --ic && dfx canister call --ic liquity send_raw_transaction && dfx canister status --ic liquity