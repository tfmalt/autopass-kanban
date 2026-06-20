# Kanban Web — Containerized Quickstart

The kanban web server runs in a long-lived Docker container with the embedded Rust web server and the Vite-built React client bundled into the `kanban` binary.

## Prerequisites

| Requirement | Minimum version | Check |
|---|---|---|
| Docker Engine | 24.x | `docker --version` |
| Docker Compose plugin | v2 | `docker compose version` |
| Kanban repo cloned at | `/git/autopass-kanban` | `ls /git/autopass-kanban/Cargo.toml` |
| AutoPASS IP 2.0 repo cloned at | `/git/ip-2.0` | `ls /git/ip-2.0/.kanban/paths.json` |

## Start the web UI

```bash
cd /git/autopass-kanban
./docker-compose.up.sh
```

Open `http://localhost:3000`.

The script:

1. Builds `ip2-kanban-web:local` from `Dockerfile`.
2. Starts `aup-kanban-web-1` with `restart: always`.
3. Mounts `${KANBAN_REPO_PATH:-../ip-2.0}` as `/repo` so reads and writes use the live AutoPASS IP 2.0 checkout.
4. Runs the container with your UID/GID so markdown edits are owned by you.

To use cached layers:

```bash
cd /git/autopass-kanban
KANBAN_UID="$(id -u)" KANBAN_GID="$(id -g)" docker compose up -d aup-kanban-web
```

## Day-to-day operations

Check status:

```bash
docker ps --filter name=aup-kanban-web-1
```

Follow logs:

```bash
docker logs -f aup-kanban-web-1
```

Open a shell:

```bash
/git/autopass-kanban/docker-compose.bash.sh
```

Stop the container:

```bash
docker compose -f /git/autopass-kanban/docker-compose.yml down
```

## Architecture in brief

```
browser :3000
  -> aup-kanban-web-1
       -> /usr/local/bin/kanban
          -> embedded Rust web server
        -> /repo  <- bind mount -> /git/ip-2.0
```

| File | Purpose |
|---|---|
| `Dockerfile` | Multi-stage build: Vite web build, Rust CLI build, slim Debian runtime |
| `docker-compose.yml` | Compose service with fixed container name, UID/GID passthrough, and repo bind mount |
| `docker-compose.up.sh` | Clean build and start helper |
| `docker-compose.bash.sh` | Opens a shell in the running container |
| `entrypoint.sh` | Starts `kanban web serve` from `/repo` so git and `.kanban` config resolve correctly |

## Troubleshooting

### Port 3000 is already in use

Change both the host port and server port for this run:

```bash
KANBAN_UID="$(id -u)" KANBAN_GID="$(id -g)" KANBAN_WEB_PORT=3001 \
  docker compose -f /git/autopass-kanban/docker-compose.yml up -d aup-kanban-web
```

Then open `http://localhost:3001`.

### Markdown edits have the wrong owner

Restart with UID/GID passthrough:

```bash
cd /git/autopass-kanban
docker compose down
KANBAN_UID="$(id -u)" KANBAN_GID="$(id -g)" docker compose up -d aup-kanban-web
```

### The app cannot find `.kanban`

Check that the live repo is mounted at `/repo`:

```bash
docker inspect aup-kanban-web-1 | grep -A3 Mounts
```
