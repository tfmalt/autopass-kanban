#!/usr/bin/env bash
set -euo pipefail

# Ensure /workspace exists and has correct ownership for the non-root user
if [[ -d /workspace ]]; then
  # Get the actual UID/GID of the current user (set by docker-compose via user: field)
  current_uid=$(id -u)
  current_gid=$(id -g)
  
  # Fix ownership so the running user can read/write /workspace
  if [[ $current_uid -ne 0 ]]; then
    chown -R "$current_uid:$current_gid" /workspace 2>/dev/null || true
  fi
fi

# Start the long-lived sleep process
exec sleep infinity
