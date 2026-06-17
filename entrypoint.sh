#!/bin/sh
set -eu

if [ "$#" -gt 0 ]; then
  exec "$@"
fi

exec kanban web serve \
  --repo-root "${KANBAN_REPO_ROOT:-/repo}" \
  --host "${KANBAN_WEB_HOST:-0.0.0.0}" \
  --port "${KANBAN_WEB_PORT:-3000}"
