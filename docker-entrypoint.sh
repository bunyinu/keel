#!/bin/sh
set -e
mkdir -p /data
# Render persistent disk mounts as root; ensure app user can write SQLite.
chown -R keel:keel /data 2>/dev/null || true
exec gosu keel:keel /usr/local/bin/keel-server
