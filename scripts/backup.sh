#!/bin/sh
set -eu
umask 077
cd "$(dirname "$0")/.."
[ -f .env ] || { echo 'Missing private .env' >&2; exit 1; }
stamp=$(date -u '+%Y%m%dT%H%M%SZ')
dest="backups/$stamp"
mkdir -p "$dest"
# Both database and encryption/signing keys are required for a usable restoration.
cp .env "$dest/config.env"
docker compose exec -T postgres sh -c 'pg_dump -U "$POSTGRES_USER" -d "$POSTGRES_DB" --format=custom' > "$dest/database.dump"
[ -s "$dest/database.dump" ] || { echo 'Database backup is empty' >&2; exit 1; }
if [ -d secrets ]; then cp -R secrets "$dest/secrets"; fi
printf '%s\n' "Backup written to $dest. This directory contains secrets; encrypt it before moving off-host."
