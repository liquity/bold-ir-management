#!/bin/bash
sh fix_and_fmt.sh
cd ir_manager
cargo run --features export-api > candid.did
cd ..
cargo build --release --target wasm32-unknown-unknown --features export-api