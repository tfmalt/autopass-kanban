#!/bin/sh
set -eu

INSTALLER_VERSION="26.6.2211"
C_RESET=""
C_BRAND=""
C_GREEN=""

PREFIX=""
DRY_RUN=0
QUIET=0
YES=0
SKILLS_DIR=""

RC_FILE=""
SHELL_NAME=""

FILES_REMOVED=0
FILES_SKIPPED=0
RC_FILES_EDITED=0

cleanup() {
	:
}
trap cleanup EXIT

init_ui() {
	if [ -t 2 ] && [ -z "${NO_COLOR:-}" ] && [ "${TERM:-}" != "dumb" ]; then
		C_RESET='\033[0m'
		C_BRAND='\033[93m'
		C_GREEN='\033[32m'
	fi
}

brand() {
	printf '%bkanban%b' "$C_BRAND" "$C_RESET"
}

version() {
	printf '%b%s%b' "$C_GREEN" "$1" "$C_RESET"
}

log() {
	if [ "$QUIET" -eq 0 ]; then
		echo "- $*" >&2
	fi
}

die() {
	printf '%s-uninstaller: %s\n' "$(brand)" "$*" >&2
	exit 1
}

usage() {
	cat >&2 <<'USAGE'
Usage: sh scripts/uninstall.sh [flags]

Flags:
  --prefix <dir>    Prefix used during install (default: ~/.local/bin)
  --skills-dir <dir> Skills dir used during install (optional)
  --yes             Skip confirmation prompts
  --dry-run         Preview all actions without modifying the filesystem
  --quiet           Suppress non-error log lines

Example:
  sh scripts/uninstall.sh
  sh scripts/uninstall.sh --prefix ~/bin --dry-run
USAGE
}

parse_args() {
	while [ $# -gt 0 ]; do
		case "$1" in
			--prefix)
				if [ $# -lt 2 ]; then usage; exit 2; fi
				PREFIX="$2"; shift 2 ;;
			--skills-dir)
				if [ $# -lt 2 ]; then usage; exit 2; fi
				SKILLS_DIR="$2"; shift 2 ;;
			--yes)
				YES=1; shift ;;
			--dry-run)
				DRY_RUN=1; shift ;;
			--quiet)
				QUIET=1; shift ;;
			--help|-h)
				usage; exit 0 ;;
			*)
				usage; exit 2 ;;
		esac
	done

	if [ -z "$PREFIX" ]; then
		PREFIX="${HOME}/.local/bin"
	fi
}

resolve_path() {
	case "$1" in
		~/*) printf '%s/%s' "$HOME" "${1#~/}" ;;
		"~") printf '%s' "$HOME" ;;
		*)   printf '%s' "$1" ;;
	esac
}

detect_shell() {
	shell_path="${SHELL:-/bin/sh}"
	SHELL_NAME="${shell_path##*/}"

	case "$SHELL_NAME" in
		bash)  RC_FILE="${HOME}/.bashrc" ;;
		zsh)   RC_FILE="${HOME}/.zshrc" ;;
		ash)   RC_FILE="${HOME}/.profile" ;;
		*)     RC_FILE="" ;;
	esac
}

find_sha256_cmd() {
	if command -v sha256sum >/dev/null 2>&1; then
		echo "sha256sum"
	elif command -v shasum >/dev/null 2>&1; then
		echo "shasum -a 256"
	else
		die "no sha256 utility found (sha256sum or shasum required)"
	fi
}

compute_sha256() {
	_path="$1"
	_sha_cmd=$(find_sha256_cmd)
	_hash=$($_sha_cmd "$_path" 2>/dev/null | awk '{print $1}')
	if [ -z "$_hash" ]; then
		die "failed to compute sha256 for $_path"
	fi
	echo "$_hash"
}

validate_safe_path() {
	_path="$1"
	_label="$2"
	case "$_path" in
		*..*)
			die "$_label contains '..' which is not allowed: $_path"
			;;
	esac
}

manifest_dir() {
	_prefix_resolved=$(resolve_path "$PREFIX")
	_prefix_parent=$(dirname "$_prefix_resolved")
	printf '%s/lib/kanban' "$_prefix_parent"
}

manifest_path() {
	printf '%s/manifest.txt' "$(manifest_dir)"
}

read_manifest_entries() {
	_manifest="$1"
	if [ ! -f "$_manifest" ]; then
		return 1
	fi
	grep -v '^#' "$_manifest" 2>/dev/null | grep -v '^$' || true
}

remove_if_hash_matches() {
	_path="$1"
	_expected_hash="$2"

	if [ ! -f "$_path" ]; then
		log "! file not found, skipping: $_path"
		FILES_SKIPPED=$((FILES_SKIPPED + 1))
		return 0
	fi

	_disk_hash=$(compute_sha256 "$_path")

	if [ "$_disk_hash" != "$_expected_hash" ]; then
		log "! hash mismatch, skipping (user-edited): $_path"
		FILES_SKIPPED=$((FILES_SKIPPED + 1))
		return 0
	fi

	if [ "$DRY_RUN" -eq 1 ]; then
		echo "[dry-run] would rm $_path" >&2
		FILES_REMOVED=$((FILES_REMOVED + 1))
		return 0
	fi

	rm -f "$_path"
	log "removed: $_path"
	FILES_REMOVED=$((FILES_REMOVED + 1))
}

prompt_skill_removal() {
	_skills_dir="$1"

	if [ "$YES" -eq 1 ]; then
		return 0
	fi

	if [ ! -t 0 ]; then
		log "non-interactive: defaulting to remove skills at $_skills_dir"
		return 0
	fi

	if [ "$DRY_RUN" -eq 1 ]; then
		echo "[dry-run] would prompt: Remove $(brand) skills from $_skills_dir? [Y/n]" >&2
		return 0
	fi

	printf 'Remove %s skills from %s? [Y/n] ' "$(brand)" "$_skills_dir" > /dev/tty
	read -r _resp < /dev/tty

	case "$_resp" in
		n|N) return 1 ;;
		*)    return 0 ;;
	esac
}

strip_sentinel_lines() {
	_rc="$1"
	_sentinel="kanban-installer:"

	if [ ! -f "$_rc" ]; then
		return 0
	fi

	if grep -qF "$_sentinel" "$_rc" 2>/dev/null; then
		:
	else
		return 0
	fi

	if [ "$DRY_RUN" -eq 1 ]; then
		echo "[dry-run] would strip sentinel lines from $_rc" >&2
		return 0
	fi

	_tmp="${_rc}.$$.tmp"
	grep -vF "$_sentinel" "$_rc" > "$_tmp" 2>/dev/null || true
	mv -f "$_tmp" "$_rc"
	log "stripped sentinel lines from $_rc"
	RC_FILES_EDITED=$((RC_FILES_EDITED + 1))

	if [ ! -s "$_rc" ]; then
		rm -f "$_rc"
		log "removed empty rc file: $_rc"
	fi
}

uninstall_files() {
	_manifest="$1"
	_entries=$(read_manifest_entries "$_manifest")

	if [ -z "$_entries" ]; then
		log "no entries in manifest; nothing to remove"
		return 0
	fi

	_tmp="/tmp/kanban-uninstall-manifest.$$"
	printf '%s\n' "$_entries" > "$_tmp"

	while IFS= read -r _line; do
		[ -z "$_line" ] && continue
		_path=$(printf '%s' "$_line" | awk -F'\t' '{print $1}')
		_hash=$(printf '%s' "$_line" | awk -F'\t' '{print $2}')

		_path=$(resolve_path "$_path")
		case "$_path" in
			*..*)
				log "! refusing to follow unsafe path: $_path"
				FILES_SKIPPED=$((FILES_SKIPPED + 1))
				continue
				;;
		esac

		remove_if_hash_matches "$_path" "$_hash"
	done < "$_tmp"

	rm -f "$_tmp"
}

remove_manifest_dir() {
	_manifest_dir=$(manifest_dir)

	if [ "$DRY_RUN" -eq 1 ]; then
		echo "[dry-run] would rm -rf $_manifest_dir" >&2
		return 0
	fi

	if [ -d "$_manifest_dir" ]; then
		rm -rf "$_manifest_dir"
		log "removed manifest directory: $_manifest_dir"
	fi
}

main() {
	init_ui
	parse_args "$@"

	log "$(brand)-uninstaller $(version "v${INSTALLER_VERSION}")"
	log "prefix: $PREFIX"
	if [ "$DRY_RUN" -eq 1 ]; then
		log "mode: dry-run (no filesystem changes)"
	fi

	detect_shell

	if [ -n "$RC_FILE" ]; then
		log "detected shell: $SHELL_NAME (rc: $RC_FILE)"
	fi

	_manifest=$(manifest_path)

	if [ ! -f "$_manifest" ]; then
		log "no manifest found at $_manifest"
		_rc_expanded=$(resolve_path "$RC_FILE")
		if [ -f "$_rc_expanded" ]; then
			strip_sentinel_lines "$_rc_expanded"
		fi
		log "nothing else to remove (no manifest)"
		exit 0
	fi

	_old_skills_dir=$(grep '^# skills-dir:' "$_manifest" 2>/dev/null | sed 's/^# skills-dir: //' || echo "")

	if [ -n "$_old_skills_dir" ]; then
		if prompt_skill_removal "$_old_skills_dir"; then
			log "removing skills from $_old_skills_dir"
			SKILLS_DIR="$_old_skills_dir"
		else
			log "skill removal declined"
			SKILLS_DIR=""
		fi
	fi

	uninstall_files "$_manifest"

	_rc_expanded=$(resolve_path "$RC_FILE")
	if [ -f "$_rc_expanded" ]; then
		strip_sentinel_lines "$_rc_expanded"
	fi

	remove_manifest_dir

	echo ""
	echo "Uninstall summary:"
	echo "  files removed:  $FILES_REMOVED"
	echo "  files skipped:  $FILES_SKIPPED"
	echo "  rc files edited: $RC_FILES_EDITED"

	if [ "$DRY_RUN" -eq 1 ]; then
		log "dry-run complete — no changes were made"
	else
		log "uninstall complete"
	fi
}

main "$@"
