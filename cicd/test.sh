#!/bin/sh
set -eu

cargo test --features sync --lib --test test_sync --bin openapi_from_code
cargo test --features async_tokio --lib --test test_async_tokio --bin openapi_from_code
cargo test --features async_std --lib --test test_async_std --bin openapi_from_code
cargo test --features async_smol --lib --test test_async_smol --bin openapi_from_code
