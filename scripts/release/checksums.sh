#!/bin/sh
set -eu

C_RESET=""
C_BRAND=""
if [ -t 2 ] && [ -z "${NO_COLOR:-}" ] && [ "${TERM:-}" != "dumb" ]; then
	C_RESET='\033[0m'
	C_BRAND='\033[93m'
fi

brand() {
	printf '%bkanban%b' "$C_BRAND" "$C_RESET"
}

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
		printf '%s-release: no sha256 utility found (sha256sum or shasum required)\n' "$(brand)" >&2
		exit 1
	fi
}

SHA_CMD=$(find_sha256_cmd)
TMPFILE=$(mktemp /tmp/kanban-checksums.XXXXXX)
trap 'rm -f "$TMPFILE"' EXIT

for archive in "$@"; do
	if [ ! -f "$archive" ]; then
		printf '%s-release: archive not found: %s\n' "$(brand)" "$archive" >&2
		exit 1
	fi

	basename="${archive##*/}"
	hash=$($SHA_CMD "$archive" 2>/dev/null | awk '{print $1}')
	if [ -z "$hash" ]; then
		printf '%s-release: failed to compute sha256 for %s\n' "$(brand)" "$archive" >&2
		exit 1
	fi

	printf '%s  %s\n' "$hash" "$basename" >> "$TMPFILE"
done

sort -k2 "$TMPFILE"
