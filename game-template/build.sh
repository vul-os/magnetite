#!/bin/bash
set -e

cargo build --target wasm32-unknown-unknown --release
wasm-bindgen --target web --out-dir ./dist --out-name game target/wasm32-unknown-unknown/release/magnetite_game_template.wasm
