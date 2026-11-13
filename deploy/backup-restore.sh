#!/bin/sh
# Dump and restore one disposable database. Refuses any name without ferrioxid_test.
set -eu
if [ "${1:-}" = "" ] || [ "${2:-}" = "" ]; then
  echo "usage: backup-restore.sh SOURCE_URL TARGET_URL" >&2
  exit 2
fi
case "$1" in
  *ferrioxid_test*) ;;
  *)
    echo "refusing: source URL must contain ferrioxid_test" >&2
    exit 2
    ;;
esac
case "$2" in
  *ferrioxid_test*) ;;
  *)
    echo "refusing: target URL must contain ferrioxid_test" >&2
    exit 2
    ;;
esac
dump="$(mktemp)"
trap 'rm -f "$dump"' EXIT
pg_dump --format=custom --file "$dump" --dbname "$1"
pg_restore --no-owner --dbname "$2" "$dump"
echo "restore finished"
