#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
# Only this Compose project is removed. Build caches remain for subsequent runs.
cleanup() { docker compose -f docker-compose.verify.yml down --remove-orphans; }
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
docker compose -f docker-compose.verify.yml run --rm rust
docker compose -f docker-compose.verify.yml run --rm frontend
