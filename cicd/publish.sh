#!/bin/sh
set -eu

CRATE_NAME="$(awk -F'"' '/^name = "/ { print $2; exit }' Cargo.toml)"
CRATE_VERSION="$(awk -F'"' '/^version = "/ { print $2; exit }' Cargo.toml)"
MODE="${1:-}"

if [ -z "$CRATE_NAME" ] || [ -z "$CRATE_VERSION" ]; then
  echo "Could not read package name/version from Cargo.toml."
  exit 1
fi

if [ "$MODE" = "--dry-run" ]; then
  cargo publish --dry-run --features sync
  exit $?
fi

if [ -n "$MODE" ]; then
  echo "Usage: sh cicd/publish.sh [--dry-run]"
  exit 2
fi

STATUS="$(curl -sS -o /dev/null -w '%{http_code}' -A 'pageboy-ci' "https://crates.io/api/v1/crates/$CRATE_NAME/$CRATE_VERSION")"

case "$STATUS" in
  200)
    echo "$CRATE_NAME $CRATE_VERSION is already published. Skipping."
    exit 0
    ;;
  404)
    ;;
  *)
    echo "Could not verify crates.io version. HTTP $STATUS."
    exit 1
    ;;
esac

if [ -z "${CARGO_REGISTRY_TOKEN:-}" ]; then
  echo "CARGO_REGISTRY_TOKEN is required to publish."
  exit 1
fi

cargo publish --features sync
