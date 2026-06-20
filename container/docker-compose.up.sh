#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$script_dir"

compose_file="docker-compose.kanban.yml"
service_name="aup-kanban"

if [[ ! -f "$compose_file" ]]; then
  printf 'Error: missing %s\n' "$compose_file" >&2
  exit 1
fi

if ! docker compose -f "$compose_file" config --services | grep -qx "$service_name"; then
  printf 'Error: service "%s" not found in %s\n' "$service_name" "$compose_file" >&2
  exit 1
fi

docker rm -f aup-kanban-1
docker compose -f "$compose_file" down --remove-orphans

# docker compose -f "$compose_file" build --progress plain --no-cache "$service_name"
docker compose -f "$compose_file" up -d --remove-orphans "$service_name"
printf 'Press Enter to open a shell in %s... ' "$service_name"
read -r _
docker compose -f "$compose_file" exec "$service_name" /bin/bash
