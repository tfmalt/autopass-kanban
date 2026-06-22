#!/bin/sh
set -eu

usage() {
	cat >&2 <<'USAGE'
Usage: sh scripts/release/checksums.sh <archive> [archive...]

Produces a sha256sum-format checksums file from one or more release archives.
Each archive must follow the naming convention:
  kanban-<version>-<target>.tar.gz

Output format (sha256sum compatible):
  <sha256>  <tarball>

Example:
  sh scripts/release/checksums.sh kanban-26.6.2201-x86_64-apple-darwin.tar.gz
  sh scripts/release/checksums.sh kanban-26.6.2201-*.tar.gz
USAGE
}

if [ $# -eq 0 ]; then
	usage
	exit 1
fi

case "$1" in
	--help|-h) usage; exit 0 ;;
esac

find_sha256_cmd() {
	if command -v sha256sum >/dev/null 2>&1; then
		echo "sha256sum"
	elif command -v shasum >/dev/null 2>&1; then
		echo "shasum -a 256"
	else
		echo "kanban-release: no sha256 utility found (sha256sum or shasum required)" >&2
		exit 1
	fi
}

SHA_CMD=$(find_sha256_cmd)
TMPFILE=$(mktemp /tmp/kanban-checksums.XXXXXX)
trap 'rm -f "$TMPFILE"' EXIT

for archive in "$@"; do
	if [ ! -f "$archive" ]; then
		echo "kanban-release: archive not found: $archive" >&2
		exit 1
	fi

	basename="${archive##*/}"
	hash=$($SHA_CMD "$archive" 2>/dev/null | awk '{print $1}')
	if [ -z "$hash" ]; then
		echo "kanban-release: failed to compute sha256 for $archive" >&2
		exit 1
	fi

	printf '%s  %s\n' "$hash" "$basename" >> "$TMPFILE"
done

sort -k2 "$TMPFILE"
