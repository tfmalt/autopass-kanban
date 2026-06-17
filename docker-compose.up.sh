#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$script_dir"

compose_file="docker-compose.yml"
service_name="aup-kanban-web"
kanban_uid="$(id -u)"
kanban_gid="$(id -g)"

if [[ ! -f "$compose_file" ]]; then
  printf 'Error: missing %s\n' "$compose_file" >&2
  exit 1
fi

if ! KANBAN_UID="$kanban_uid" KANBAN_GID="$kanban_gid" docker compose -f "$compose_file" config --services | grep -qx "$service_name"; then
  printf 'Error: service "%s" not found in %s\n' "$service_name" "$compose_file" >&2
  exit 1
fi

KANBAN_UID="$kanban_uid" KANBAN_GID="$kanban_gid" docker compose --progress plain -f "$compose_file" build --no-cache "$service_name"
KANBAN_UID="$kanban_uid" KANBAN_GID="$kanban_gid" docker compose -f "$compose_file" up -d --remove-orphans "$service_name"

printf 'kanban-web is running at http://localhost:%s\n' "${KANBAN_WEB_PORT:-3000}"
