# Kanban CLI — Containerized Quickstart

The `kanban` CLI and web UI run inside a long-lived Docker container. The binary is never compiled locally. This guide gets you from zero to a working `kanban` command and web UI in one terminal session.

---

## Prerequisites

| Requirement | Minimum version | Check |
|---|---|---|
| Docker Engine | 24.x | `docker --version` |
| Docker Compose plugin | v2 | `docker compose version` |
| Bash | 5.x | `bash --version` |
| Kanban repo cloned at | `/git/autopass-kanban` | `ls /git/autopass-kanban/Cargo.toml` |
| AutoPASS IP 2.0 repo cloned at | `/git/ip-2.0` | `ls /git/ip-2.0/.kanban/settings.json` |

> **Default project path:** the compose file mounts the sibling checkout `../ip-2.0` by default. Set `KANBAN_REPO_PATH=/path/to/project` to use another markdown backlog repository.

---

## Step 1 — Build and start the container

Run this **once** from the repo root (or after changes to Rust or web source):

```bash
cd /git/autopass-kanban
./docker-compose.up.sh
```

This script:
1. Builds `ip2-kanban-web:local` from `Dockerfile` (multi-stage: Vite web build → Rust CLI build → slim Debian runtime — takes ~2 min on first run, always `--no-cache`)
2. Starts the `aup-kanban-web-1` container with `restart: always`
3. Prints `kanban-web is running at http://localhost:3000`

Open `http://localhost:3000` to use the web UI.

To start without rebuilding (uses cached layers):

```bash
cd /git/autopass-kanban
KANBAN_UID="$(id -u)" KANBAN_GID="$(id -g)" docker compose up -d aup-kanban-web
```

---

## Step 2 — Add `kanban` to your PATH

Add one block to `~/.bashrc` (or `~/.zshrc`):

```bash
# kanban CLI wrapper
if [[ -d /git/autopass-kanban/container ]]; then
  export PATH="/git/autopass-kanban/container:$PATH"
fi
```

Then reload:

```bash
source ~/.bashrc
```

Verify:

```bash
which kanban        # /git/autopass-kanban/container/kanban
which kb            # /git/autopass-kanban/container/kb
kanban --version    # kanban 26.6.2115
```

---

## Step 3 — Use `kanban`

```bash
kanban --help
kanban sprint
kanban board
kb sprint sync      # kb is a short alias for kanban
```

The wrapper (`container/kanban`) automatically:
- Starts `aup-kanban-web-1` if it is not running (`docker compose up -d`)
- Forwards all arguments and flags to the binary inside the container
- Mounts the project repo (`/git/ip-2.0` by default) as `/repo` inside the container so all backlog file reads/writes go to the real checkout

---

## Day-to-day operations

### Check container status

```bash
docker ps --filter name=aup-kanban-web-1
```

### Open a shell inside the container

```bash
/git/autopass-kanban/docker-compose.bash.sh
# or directly:
docker exec -it aup-kanban-web-1 /bin/sh
```

### Rebuild after Rust or web source changes

```bash
cd /git/autopass-kanban
./docker-compose.up.sh
```

The script always passes `--no-cache` to ensure a clean rebuild.

### Stop the container

```bash
docker compose -f /git/autopass-kanban/docker-compose.yml down
```

The container is configured with `restart: always`, so it restarts automatically on Docker daemon startup. Stop it explicitly only when needed.

---

## Architecture

```
browser :3000
  -> aup-kanban-web-1  (ip2-kanban-web:local)
       -> /usr/local/bin/kanban web serve
          -> embedded Rust web server
       -> /repo  <- bind mount -> /git/ip-2.0

host shell
  -> container/kanban  (wrapper)
       -> docker compose exec aup-kanban-web kanban <args>
          -> /usr/local/bin/kanban
       -> /repo  <- bind mount -> /git/ip-2.0
```

| File | Purpose |
|---|---|
| `Dockerfile` | Multi-stage build: Vite web build, Rust CLI build, slim Debian runtime |
| `docker-compose.yml` | Compose service with fixed container name, UID/GID passthrough, and repo bind mount |
| `docker-compose.up.sh` | Clean build (`--no-cache`) and start helper |
| `docker-compose.bash.sh` | Opens a shell (`/bin/sh`) in the running container |
| `entrypoint.sh` | Starts `kanban web serve` from `/repo` so git and `.kanban` config resolve correctly |
| `container/kanban` | Host-side wrapper: starts container if needed, then `docker compose exec` |
| `container/kb` | Thin alias: calls `kanban "$@"` |

---

## Architecture in brief

```
~/.bashrc PATH entry
  └─ /git/autopass-kanban/container/kanban   (wrapper script)
       └─ docker compose exec aup-kanban-1 kanban  (binary inside container)
            └─ /workspace  ←──── volume mount ────  /git/ip-2.0
```

| File | Purpose |
|---|---|
| `container/Dockerfile.kanban` | Multi-stage build: Rust builder → `debian:bookworm-slim` runtime |
| `container/docker-compose.kanban.yml` | Compose service; sets `restart: always`, user UID/GID passthrough, volume mount |
| `container/kanban` | Wrapper script; starts container if needed, execs `kanban` inside it |
| `container/kb` | Thin alias — delegates to `kanban "$@"` |
| `container/docker-compose.up.sh` | Build + start helper; drops into container shell |
| `container/docker-compose.bash.sh` | Opens a shell in the already-running container |
| `bin/kanban` | **Original local runner** — runs the local debug binary or `cargo run`; used by team members not using Docker |

---

## Troubleshooting

### `docker: command not found`

Install Docker Desktop (macOS/Windows) or Docker Engine (Linux). See [docs.docker.com/engine/install](https://docs.docker.com/engine/install/).

### `kanban` runs but edits are not saved

Check that the container mounts the live repo checkout:

```bash
docker inspect aup-kanban-1 | grep -A3 Mounts
# Source should be /git/ip-2.0 unless KANBAN_REPO_PATH overrides it
```

### Container exits immediately

The image uses `ENTRYPOINT ["sleep"] CMD ["infinity"]`. If the container is not running:

```bash
docker compose -f /git/autopass-kanban/container/docker-compose.kanban.yml up -d
docker ps --filter name=aup-kanban-1
```

### Permission errors on backlog files

**Most common cause:** The volume mount (`/workspace`) has incompatible ownership or the container was started with a different UID/GID.

**Automatic fix:** The container's entrypoint script automatically corrects `/workspace` ownership to match the running user when the container starts. **No manual action required** — just ensure the container is fresh:

```bash
docker compose -f /git/autopass-kanban/container/docker-compose.kanban.yml down
docker compose -f /git/autopass-kanban/container/docker-compose.kanban.yml up -d aup-kanban
```

**Manual fix (if needed):** If the container is still unable to write:

```bash
sudo chown -R "$(id -u):$(id -g)" /git/ip-2.0
```

The container passes your UID/GID via `KANBAN_UID`/`KANBAN_GID` environment variables in the wrapper script (`container/kanban`).
