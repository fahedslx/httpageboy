#!/bin/sh
set -eu

cargo check --features sync
cargo test --features sync --lib
cargo test --features sync --test test_sync

cargo check --features async_tokio
cargo test --features async_tokio --lib
cargo test --features async_tokio --test test_async_tokio

cargo check --features async_std
cargo test --features async_std --lib
cargo test --features async_std --test test_async_std

cargo check --features async_smol
cargo test --features async_smol --lib
cargo test --features async_smol --test test_async_smol

cargo test --features sync --bin openapi_from_code
