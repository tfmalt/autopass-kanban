#!/bin/sh
set -eu

INSTALLER_VERSION="26.6.2211"
GITHUB_REPO="tfmalt/autopass-kanban"
UMASK_SAVED=""
RC_FILE=""
SHELL_NAME=""
BINARY_NAME="kanban"
C_RESET=""
C_BOLD=""
C_DIM=""
C_RED=""
C_GREEN=""
C_YELLOW=""
C_BLUE=""
C_VALUE=""
C_BRAND=""
ICON_STEP="→"
ICON_OK="✓"
ICON_WARN="⚠"
ICON_ERROR="✗"
ICON_PROMPT="?"

BINARY=""
PREFIX=""
DRY_RUN=0
QUIET=0
SKILLS_DIR=""
SKILLS_DIR_EXPLICIT=0
NO_SKILLS=0
ADD_PATH=""
COMPLETIONS=""
YES=0
SKILL_MANIFEST_ENTRIES=""
REPO_ROOT=""
FORCE=0
PLANNED_BIN_DST=""
PLANNED_ENTRIES=""
OLD_MANIFEST_ENTRIES=""
REMOTE=0
REMOTE_VERSION=""
REMOTE_CHANNEL=""
OFFLINE=0
CACHE_DIR=""
TARGET_TRIPLE=""
TARBALL_URL=""
CHECKSUMS_URL=""
REMOTE_ARTIFACT_VERSION=""
EXTRACTED_DIR=""
LOG_FILE=""
LOG_READY=0
PROGRESS_STARTED=0
PROGRESS_LINE_OPEN=0
PROGRESS_CURRENT=0
PROGRESS_TOTAL=7
PROGRESS_TITLE=""
EXISTING_MANIFEST=""
EXISTING_MANIFEST_PATH_SELECTED=""
EXISTING_MANIFEST_COMPLETIONS_SELECTED=""
EXISTING_MANIFEST_SKILLS_DIR=""
EXISTING_MANIFEST_HAS_SKILLS=0

cleanup() {
	if [ -n "${UMASK_SAVED:-}" ]; then
		umask "$UMASK_SAVED" 2>/dev/null || true
	fi
}

trap cleanup EXIT

init_ui() {
	if [ -t 2 ] && [ -z "${NO_COLOR:-}" ] && [ "${TERM:-}" != "dumb" ]; then
		C_RESET='\033[0m'
		C_BOLD='\033[1m'
		C_DIM='\033[2m'
		C_RED='\033[31m'
		C_GREEN='\033[32m'
		C_YELLOW='\033[33m'
		C_BLUE='\033[34m'
		C_VALUE='\033[35m'
		C_BRAND='\033[93m'
	fi

	if [ -n "${KANBAN_INSTALL_ASCII:-}" ]; then
		ICON_STEP=">"; ICON_OK="ok"; ICON_WARN="!"; ICON_ERROR="x"; ICON_PROMPT="?"
	fi
}

log() {
	if [ "$LOG_READY" -eq 1 ]; then
		printf '%s %s\n' "$(iso8601_now)" "$*" >> "$LOG_FILE"
	elif [ "$QUIET" -eq 0 ]; then
		printf '%b%s%b %s\n' "$C_BLUE" "$ICON_STEP" "$C_RESET" "$*" >&2
	fi
}

detail() {
	log "$*"
}

ok() {
	if [ "$QUIET" -eq 0 ]; then
		printf '%b%s%b %s\n' "$C_GREEN" "$ICON_OK" "$C_RESET" "$*" >&2
	fi
}

warn() {
	log "warning: $*"
}

value() {
	printf '%b%s%b' "$C_VALUE" "$1" "$C_RESET"
}

brand() {
	printf '%bkanban%b' "$C_BRAND" "$C_RESET"
}

version() {
	printf '%b%s%b' "$C_GREEN" "$1" "$C_RESET"
}

init_log_file() {
	_stamp=$(date '+%Y%m%d-%H%M%S' 2>/dev/null || date '+%s' 2>/dev/null || echo "unknown")
	if [ "$DRY_RUN" -eq 1 ]; then
		_log_dir="${TMPDIR:-/tmp}"
	else
		_log_dir="${CACHE_DIR:-${HOME}/.cache/kanban}"
		mkdir -p "$_log_dir" 2>/dev/null || _log_dir="${TMPDIR:-/tmp}"
	fi
	LOG_FILE="${_log_dir%/}/install-${_stamp}-$$.log"
	: > "$LOG_FILE" 2>/dev/null || {
		LOG_FILE="${TMPDIR:-/tmp}/install-${_stamp}-$$.log"
		: > "$LOG_FILE" 2>/dev/null || die "failed to create install log"
	}
	LOG_READY=1
	log "$(brand) installer $(version "v${INSTALLER_VERSION}")"
}

progress_bar() {
	_width=24
	_filled=$((PROGRESS_CURRENT * _width / PROGRESS_TOTAL))
	_i=0
	_bar=""
	while [ "$_i" -lt "$_filled" ]; do
		_bar="${_bar}█"
		_i=$((_i + 1))
	done
	while [ "$_i" -lt "$_width" ]; do
		_bar="${_bar}░"
		_i=$((_i + 1))
	done
	printf '%s' "$_bar"
}

progress_render() {
	_bar=$(progress_bar)
	printf '\r%b%s%b %s [%s]' "$C_BLUE" "$ICON_STEP" "$C_RESET" "$PROGRESS_TITLE" "$_bar" >&2
	PROGRESS_LINE_OPEN=1
}

progress_break_for_prompt() {
	if [ "$QUIET" -eq 0 ] && [ "$PROGRESS_LINE_OPEN" -eq 1 ] && [ -t 2 ]; then
		printf '\n' >&2
		PROGRESS_LINE_OPEN=0
	fi
}

progress_start() {
	PROGRESS_TITLE="$1"
	PROGRESS_STARTED=1
	if [ "$QUIET" -eq 0 ]; then
		if [ -t 2 ]; then
			progress_render
		else
			printf '%b%s%b %s [%s]\n' "$C_BLUE" "$ICON_STEP" "$C_RESET" "$PROGRESS_TITLE" "$(progress_bar)" >&2
		fi
	fi
}

progress_step() {
	_label="$1"
	if [ "$PROGRESS_CURRENT" -lt "$PROGRESS_TOTAL" ]; then
		PROGRESS_CURRENT=$((PROGRESS_CURRENT + 1))
	fi
	log "progress: $_label"
	if [ "$QUIET" -eq 0 ] && [ -t 2 ]; then
		progress_render
	fi
}

progress_finish() {
	_status="$1"
	PROGRESS_CURRENT=$PROGRESS_TOTAL
	if [ "$QUIET" -eq 0 ]; then
		if [ -t 2 ] && [ "$PROGRESS_STARTED" -eq 1 ]; then
			progress_render
			printf '\n' >&2
			PROGRESS_LINE_OPEN=0
		fi
		printf '%b%s%b %s install log: %s\n' "$C_GREEN" "$ICON_OK" "$C_RESET" "$_status" "$(value "$LOG_FILE")" >&2
	fi
}

display_install_version() {
	if [ -n "${REMOTE_ARTIFACT_VERSION:-}" ]; then
		printf 'v%s' "$REMOTE_ARTIFACT_VERSION"
		return 0
	fi

	if [ -n "${REMOTE_VERSION:-}" ] && [ "$REMOTE_VERSION" != "latest" ]; then
		printf '%s' "$REMOTE_VERSION"
		return 0
	fi

	if [ -n "${BINARY:-}" ] && [ -x "$BINARY" ]; then
		_bin_ver=$(get_binary_version "$BINARY")
		case "$_bin_ver" in
			""|unknown|absent|non-executable)
				;;
			*)
				printf 'v%s' "$_bin_ver"
				return 0
				;;
		esac
	fi

	printf 'v%s' "$INSTALLER_VERSION"
}

die() {
	printf '%b%s %s-installer:%b %s\n' "$C_RED" "$ICON_ERROR" "$(brand)" "$C_RESET" "$*" >&2
	if [ -n "${LOG_FILE:-}" ]; then
		printf '%s install log: %s\n' "$ICON_ERROR" "$LOG_FILE" >&2
	fi
	exit 1
}

usage() {
	cat >&2 <<'USAGE'
Usage: sh scripts/install.sh [--binary <path>] [flags]

Flags:
  --binary <path>   Path to the prebuilt kanban binary (local install mode)
  --prefix <dir>    Install directory for the binary (default: ~/.local/bin)
  --skills-dir <dir>  Install agent skills to <dir> (skips discovery and prompt)
  --no-skills       Skip agent skill installation entirely
  --add-path        Add the install directory to the detected shell profile
  --no-add-path     Do not edit the shell profile PATH
  --completions     Install shell completions for the detected shell
  --no-completions  Skip shell completion installation
  --yes             Accept all defaults without prompting
  --force           Skip safety prompts (downgrade, local-edit detection)
  --dry-run         Preview all actions without modifying the filesystem
  --version <tag>   Install a specific release from remote (e.g. --version v26.6.2201)
  --channel main    Install from main/nightly channel (warned)
  --offline         Use cached artifacts only, no network
  --cache-dir <dir> Override download cache dir (default: ~/.cache/kanban)
  --quiet           Suppress non-error log lines

Local install (requires --binary):
  sh scripts/install.sh --binary ./target/release/kanban

Remote install:
  curl -fsSL https://raw.githubusercontent.com/tfmalt/autopass-kanban/main/scripts/install.sh | bash
  curl -fsSL https://raw.githubusercontent.com/tfmalt/autopass-kanban/main/scripts/install.sh | bash -s -- --version v26.6.2201
  sh scripts/install.sh --version v26.6.2201
USAGE
}

parse_args() {
		while [ $# -gt 0 ]; do
		case "$1" in
			--binary)
				if [ $# -lt 2 ]; then
					usage
					exit 2
				fi
				BINARY="$2"
				shift 2
				;;
			--prefix)
				if [ $# -lt 2 ]; then
					usage
					exit 2
				fi
				PREFIX="$2"
				shift 2
				;;
			--skills-dir)
				if [ $# -lt 2 ]; then
					usage
					exit 2
				fi
				SKILLS_DIR="$2"
				SKILLS_DIR_EXPLICIT=1
				shift 2
				;;
			--no-skills)
				NO_SKILLS=1
				shift
				;;
			--add-path)
				ADD_PATH=1
				shift
				;;
			--no-add-path)
				ADD_PATH=0
				shift
				;;
			--completions|--completion)
				COMPLETIONS=1
				shift
				;;
			--no-completions|--no-completion)
				COMPLETIONS=0
				shift
				;;
			--yes)
				YES=1
				shift
				;;
			--force)
				FORCE=1
				shift
				;;
			--version)
				if [ $# -lt 2 ]; then
					usage
					exit 2
				fi
				REMOTE=1
				REMOTE_VERSION="$2"
				shift 2
				;;
			--channel)
				if [ $# -lt 2 ]; then
					usage
					exit 2
				fi
				REMOTE=1
				REMOTE_CHANNEL="$2"
				shift 2
				;;
			--offline)
				OFFLINE=1
				shift
				;;
			--cache-dir)
				if [ $# -lt 2 ]; then
					usage
					exit 2
				fi
				CACHE_DIR="$2"
				shift 2
				;;
			--dry-run)
				DRY_RUN=1
				shift
				;;
			--quiet)
				QUIET=1
				shift
				;;
			--help|-h)
				usage
				exit 0
				;;
			*)
				usage
				exit 2
				;;
		esac
	done

	if [ -z "$BINARY" ] && [ "$REMOTE" -eq 0 ]; then
		REMOTE=1
	fi

	if [ -z "$PREFIX" ]; then
		PREFIX="${HOME}/.local/bin"
	fi
}

resolve_path() {
	case "$1" in
		~/*)
			printf '%s/%s' "$HOME" "${1#~/}"
			;;
		"~")
			printf '%s' "$HOME"
			;;
		*)
			printf '%s' "$1"
			;;
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

dir_contains_path() {
	_dir="$1"
	_target="$2"
	if ! [ -f "$_dir" ]; then
		return 1
	fi
	grep -qF "$_target" "$_dir" 2>/dev/null
}

canonical_dir() {
	_dir="$1"
	case "$_dir" in
		/*) printf '%s' "$_dir" ;;
		~/*) printf '%s/%s' "$HOME" "${_dir#~/}" ;;
		"~") printf '%s' "$HOME" ;;
		*)  printf '%s/%s' "$(pwd)" "$_dir" ;;
	esac
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

can_prompt() {
	[ "$YES" -eq 0 ] && [ -z "${KANBAN_INSTALL_NONINTERACTIVE:-}" ] && [ -r /dev/tty ] && [ -w /dev/tty ]
}

prompt_yes_no() {
	_question="$1"
	_default="$2"

	if [ "$YES" -eq 1 ]; then
		return 0
	fi

	if [ "$DRY_RUN" -eq 1 ]; then
		detail "[dry-run] would prompt: $_question; assuming yes for preview"
		return 0
	fi

	if ! can_prompt; then
		return 1
	fi

	case "$_default" in
		yes) _suffix="[Y/n]" ;;
		*)   _suffix="[y/N]" ;;
	esac

	progress_break_for_prompt
	printf '%b%s%b %s %s ' "$C_YELLOW" "$ICON_PROMPT" "$C_RESET" "$_question" "$_suffix" > /dev/tty
	read -r _resp < /dev/tty

	case "$_resp" in
		y|Y|yes|YES) return 0 ;;
		n|N|no|NO) return 1 ;;
		"") [ "$_default" = "yes" ] ;;
		*) return 1 ;;
	esac
}

find_repo_root() {
	_script_dir="$(cd "$(dirname "$0")" && pwd)"
	REPO_ROOT="${REPO_ROOT:-$(cd "$_script_dir/.." && pwd)}"
}

detect_target() {
	_os="$(uname -s)"
	_arch="$(uname -m)"

	case "$_os" in
		Darwin)
			case "$_arch" in
				arm64) echo "aarch64-apple-darwin" ;;
				x86_64) echo "x86_64-apple-darwin" ;;
				*) return 1 ;;
			esac
			;;
		Linux)
			case "$_arch" in
				x86_64|aarch64)
					if [ -f /lib/ld-musl-*.so.1 ] || ldd /bin/sh 2>/dev/null | grep -q musl; then
						_libc="musl"
					else
						_libc="gnu"
					fi
					echo "${_arch}-unknown-linux-${_libc}"
					;;
				*) return 1 ;;
			esac
			;;
		MINGW*|MSYS*|CYGWIN*)
			case "$_arch" in
				x86_64|amd64) echo "x86_64-pc-windows-msvc" ;;
				aarch64|arm64) echo "aarch64-pc-windows-msvc" ;;
				*) return 1 ;;
			esac
			;;
		*) return 1 ;;
	esac
}

download_stdout() {
	_url="$1"
	if command -v curl >/dev/null 2>&1; then
		curl -fsSL "$_url"
	elif command -v wget >/dev/null 2>&1; then
		wget -qO- "$_url"
	else
		die "no downloader found (curl or wget required)"
	fi
}

resolve_latest_version() {
	if [ -n "${GITHUB_LATEST_TAG:-}" ]; then
		printf '%s' "$GITHUB_LATEST_TAG"
		return 0
	fi

	_api_base="${GITHUB_API_BASE:-https://api.github.com/repos/${GITHUB_REPO}}"
	_tag=$(download_stdout "${_api_base}/releases/latest" | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | sed -n '1p')
	if [ -z "$_tag" ]; then
		die "failed to resolve latest GitHub release for ${GITHUB_REPO}; pass --version v<version>"
	fi
	printf '%s' "$_tag"
}

resolve_remote_source() {
	if [ -z "${GITHUB_REPO_BASE:-}" ]; then
		RELEASE_BASE="https://github.com/${GITHUB_REPO}/releases/download"
	else
		RELEASE_BASE="$GITHUB_REPO_BASE"
	fi

	if [ -z "${REMOTE_VERSION:-}" ] && [ -z "${REMOTE_CHANNEL:-}" ]; then
		REMOTE_VERSION=$(resolve_latest_version)
		log "resolved latest GitHub release: $(value "$REMOTE_VERSION")"
	fi

	if [ "${REMOTE_VERSION:-}" = "latest" ]; then
		REMOTE_VERSION=$(resolve_latest_version)
		log "resolved latest GitHub release: $(value "$REMOTE_VERSION")"
	fi

	if [ -n "${REMOTE_VERSION:-}" ]; then
		_ver="${REMOTE_VERSION#v}"
		TARBALL_URL="${RELEASE_BASE}/v${_ver}/kanban-${_ver}-${TARGET_TRIPLE}.tar.gz"
		CHECKSUMS_URL="${RELEASE_BASE}/v${_ver}/kanban-${_ver}-checksums.txt"
		REMOTE_ARTIFACT_VERSION="$_ver"
		return 0
	fi

	if [ -n "${REMOTE_CHANNEL:-}" ]; then
		die "remote channel 'main' requires a published release; use --version or omit it to install the latest release"
	fi

	die "no remote version specified; use --version <tag>"
}

fetch_and_verify() {
	_cache="${CACHE_DIR:-${HOME}/.cache/kanban}"
	_tarball_name="kanban-${REMOTE_ARTIFACT_VERSION}-${TARGET_TRIPLE}.tar.gz"
	_checksums_name="kanban-${REMOTE_ARTIFACT_VERSION}-checksums.txt"
	_tarball_cache="${_cache}/${_tarball_name}"
	_checksums_cache="${_cache}/${_checksums_name}"

	if [ "$DRY_RUN" -eq 1 ]; then
		detail "[dry-run] would download $_tarball_name from $RELEASE_BASE"
		detail "[dry-run] would verify sha256 against $_checksums_name"
		EXTRACTED_DIR="${_cache}/kanban-${REMOTE_ARTIFACT_VERSION}-${TARGET_TRIPLE}"
		return 0
	fi

	mkdir -p "$_cache"

	if [ "$OFFLINE" -eq 0 ]; then
		if [ ! -f "$_tarball_cache" ] || [ ! -f "$_checksums_cache" ]; then
			log "downloading $(value "$TARBALL_URL")"
			if command -v curl >/dev/null 2>&1; then
				curl -fsSL "$TARBALL_URL" -o "$_tarball_cache" || die "failed to download tarball"
				curl -fsSL "$CHECKSUMS_URL" -o "$_checksums_cache" || die "failed to download checksums"
			elif command -v wget >/dev/null 2>&1; then
				wget -q "$TARBALL_URL" -O "$_tarball_cache" || die "failed to download tarball"
				wget -q "$CHECKSUMS_URL" -O "$_checksums_cache" || die "failed to download checksums"
			else
				die "no downloader found (curl or wget required)"
			fi
		fi
	else
		if [ ! -f "$_tarball_cache" ]; then
			die "offline mode: cached tarball not found at $_tarball_cache"
		fi
		if [ ! -f "$_checksums_cache" ]; then
			die "offline mode: cached checksums not found at $_checksums_cache"
		fi
	fi

	_expected=$(grep "$_tarball_name" "$_checksums_cache" 2>/dev/null | awk '{print $1}')
	if [ -z "$_expected" ]; then
		die "checksums file does not list $_tarball_name"
	fi

	_actual=$(compute_sha256 "$_tarball_cache")
	if [ "$_actual" != "$_expected" ]; then
		die "checksum mismatch for $_tarball_name: expected=$_expected actual=$_actual"
	fi
	log "checksum verified: $(value "$_tarball_name")"

	EXTRACTED_DIR="${_cache}/kanban-${REMOTE_ARTIFACT_VERSION}-${TARGET_TRIPLE}"
	rm -rf "$EXTRACTED_DIR"
	mkdir -p "$EXTRACTED_DIR"
	tar -xzf "$_tarball_cache" -C "$EXTRACTED_DIR"
	log "extracted to $(value "$EXTRACTED_DIR")"
}

iso8601_now() {
	date '+%Y-%m-%dT%H:%M:%S%z' 2>/dev/null || date '+%Y-%m-%dT%H:%M:%S' 2>/dev/null || echo "unknown"
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

load_existing_manifest_state() {
	_manifest=$(manifest_path)
	EXISTING_MANIFEST=""
	EXISTING_MANIFEST_PATH_SELECTED=""
	EXISTING_MANIFEST_COMPLETIONS_SELECTED=""
	EXISTING_MANIFEST_SKILLS_DIR=""
	EXISTING_MANIFEST_HAS_SKILLS=0

	if [ ! -f "$_manifest" ]; then
		return 0
	fi

	EXISTING_MANIFEST="$_manifest"
	EXISTING_MANIFEST_PATH_SELECTED=$(sed -n 's/^# path-installed: //p' "$_manifest" 2>/dev/null | sed -n '1p')
	EXISTING_MANIFEST_COMPLETIONS_SELECTED=$(sed -n 's/^# completions-installed: //p' "$_manifest" 2>/dev/null | sed -n '1p')
	EXISTING_MANIFEST_SKILLS_DIR=$(sed -n 's/^# skills-dir: //p' "$_manifest" 2>/dev/null | sed -n '1p')

	if grep -q 'generated:kanban-completion-' "$_manifest" 2>/dev/null; then
		if [ -z "$EXISTING_MANIFEST_COMPLETIONS_SELECTED" ]; then
			EXISTING_MANIFEST_COMPLETIONS_SELECTED="yes"
		fi
	fi

	if grep -q 'repo:skills/' "$_manifest" 2>/dev/null; then
		EXISTING_MANIFEST_HAS_SKILLS=1
	fi

	if [ -n "$EXISTING_MANIFEST" ]; then
		log "existing install manifest found: $(value "$EXISTING_MANIFEST")"
	fi
}

get_binary_version() {
	_bin="$1"
	if [ -x "$_bin" ]; then
		"$_bin" --version 2>/dev/null | awk '{print $NF}' || echo "unknown"
	elif [ -f "$_bin" ]; then
		echo "non-executable"
	else
		echo "absent"
	fi
}

reconcile_manifest() {
	_old_manifest="$1"

	if [ ! -f "$_old_manifest" ]; then
		log "! no previous manifest found; treating as fresh install"
		return 0
	fi

	_old_entries=$(read_manifest_entries "$_old_manifest")
	_planned_entries="$PLANNED_ENTRIES"

	TO_REMOVE=""
	TO_OVERWRITE=""
	TO_ADD=""

	_old_paths=""
	_tmp_old="/tmp/kanban-reconcile-old.$$"
	_tmp_planned="/tmp/kanban-reconcile-planned.$$"

	printf '%s\n' "$_old_entries" | while IFS= read -r _line; do
		[ -z "$_line" ] && continue
		_path=$(printf '%s' "$_line" | awk -F'\t' '{print $1}')
		printf '%s\n' "$_path" >> "$_tmp_old"
	done 2>/dev/null

	printf '%s\n' "$_planned_entries" | while IFS= read -r _line; do
		[ -z "$_line" ] && continue
		_path=$(printf '%s' "$_line" | awk -F'\t' '{print $1}')
		printf '%s\n' "$_path" >> "$_tmp_planned"
	done 2>/dev/null

	[ -f "$_tmp_old" ] || touch "$_tmp_old"
	[ -f "$_tmp_planned" ] || touch "$_tmp_planned"

	while IFS= read -r _path; do
		[ -z "$_path" ] && continue
		if grep -qxF "$_path" "$_tmp_planned" 2>/dev/null; then
			TO_OVERWRITE="${TO_OVERWRITE}${_path}
"
		else
			TO_REMOVE="${TO_REMOVE}${_path}
"
		fi
	done < "$_tmp_old"

	while IFS= read -r _path; do
		[ -z "$_path" ] && continue
		if ! grep -qxF "$_path" "$_tmp_old" 2>/dev/null; then
			TO_ADD="${TO_ADD}${_path}
"
		fi
	done < "$_tmp_planned"

	rm -f "$_tmp_old" "$_tmp_planned"

	OLD_MANIFEST_ENTRIES="$_old_entries"
}

lookup_manifest_hash() {
	_path="$1"
	printf '%s' "$OLD_MANIFEST_ENTRIES" | while IFS= read -r _line; do
		[ -z "$_line" ] && continue
		_line_path=$(printf '%s' "$_line" | awk -F'\t' '{print $1}')
		if [ "$_line_path" = "$_path" ]; then
			printf '%s' "$_line" | awk -F'\t' '{print $2}'
			return
		fi
	done 2>/dev/null
}

remove_orphans() {
	if [ -z "${TO_REMOVE:-}" ]; then
		return 0
	fi

	printf '%s' "$TO_REMOVE" | while IFS= read -r _path; do
		[ -z "$_path" ] && continue
		_path=$(resolve_path "$_path")
		if [ ! -f "$_path" ]; then
			continue
		fi
		_disk_hash=$(compute_sha256 "$_path" 2>/dev/null || echo "")
		_manifest_hash=$(lookup_manifest_hash "$_path")
		if [ -n "$_manifest_hash" ] && [ "$_disk_hash" != "$_manifest_hash" ]; then
			log "! skipping orphan removal of user-edited file: $_path"
			continue
		fi
		if [ "$DRY_RUN" -eq 1 ]; then
			detail "[dry-run] would remove orphan: $_path"
		else
			rm -f "$_path"
			log "- removed orphan: $_path"
		fi
	done
}

prompt_downgrade() {
	if [ "$FORCE" -eq 1 ]; then
		log "downgrade accepted (--force)"
		return 0
	fi

	_old_ver="$1"
	_new_ver="$2"

	if [ -z "$_old_ver" ] || [ "$_old_ver" = "absent" ] || [ "$_old_ver" = "unknown" ]; then
		return 0
	fi

	if [ "$_old_ver" = "$_new_ver" ]; then
		return 0
	fi

	_a_major=$(printf '%s' "$_old_ver" | cut -d. -f1)
	_a_minor=$(printf '%s' "$_old_ver" | cut -d. -f2)
	_a_patch=$(printf '%s' "$_old_ver" | cut -d. -f3)
	_b_major=$(printf '%s' "$_new_ver" | cut -d. -f1)
	_b_minor=$(printf '%s' "$_new_ver" | cut -d. -f2)
	_b_patch=$(printf '%s' "$_new_ver" | cut -d. -f3)

	_is_downgrade=0
	if [ "$_a_major" -gt "$_b_major" ] 2>/dev/null; then
		_is_downgrade=1
	elif [ "$_a_major" -eq "$_b_major" ] 2>/dev/null && [ "$_a_minor" -gt "$_b_minor" ] 2>/dev/null; then
		_is_downgrade=1
	elif [ "$_a_major" -eq "$_b_major" ] 2>/dev/null && [ "$_a_minor" -eq "$_b_minor" ] 2>/dev/null && [ "$_a_patch" -gt "$_b_patch" ] 2>/dev/null; then
		_is_downgrade=1
	fi

	if [ "$_is_downgrade" -eq 0 ]; then
		return 0
	fi

	if [ "$YES" -eq 1 ] || ! can_prompt; then
		log "downgrade refused ($_old_ver -> $_new_ver); use --force to override"
		die "downgrade refused (old=$_old_ver new=$_new_ver)"
	fi

	if [ "$DRY_RUN" -eq 1 ]; then
		detail "[dry-run] would prompt: Downgrade from $_old_ver to $_new_ver? [y/N]"
		return 0
	fi

	progress_break_for_prompt
	printf '%b%s%b Downgrade from %s to %s? [y/N] ' "$C_YELLOW" "$ICON_PROMPT" "$C_RESET" "$_old_ver" "$_new_ver" > /dev/tty
	read -r _resp < /dev/tty

	case "$_resp" in
		y|Y|yes|YES) return 0 ;;
		*) die "downgrade refused by user" ;;
	esac
}

prompt_local_edit() {
	_path="$1"
	_manifest_hash="$2"
	_disk_hash="$3"

	if [ "$FORCE" -eq 1 ]; then
		log "local edit overwrite accepted (--force): $_path"
		return 0
	fi

	if [ "$YES" -eq 1 ] || ! can_prompt; then
		log "skipping locally edited file: $_path (use --force to overwrite)"
		return 1
	fi

	if [ "$DRY_RUN" -eq 1 ]; then
		detail "[dry-run] would prompt: Local edits to $_path will be overwritten. Continue? [y/N]"
		return 0
	fi

	if [ "$QUIET" -eq 0 ]; then
		warn "local edits detected in $_path (hash changed from manifest)"
	fi
	progress_break_for_prompt
	printf '%b%s%b Local edits to %s will be overwritten. Continue? [y/N] ' "$C_YELLOW" "$ICON_PROMPT" "$C_RESET" "$_path" > /dev/tty
	read -r _resp < /dev/tty

	case "$_resp" in
		y|Y|yes|YES) return 0 ;;
		*) return 1 ;;
	esac
}

atomic_copy() {
	_src="$1"
	_dst="$2"
	_dst_dir=$(dirname "$_dst")
	_tmp="${_dst_dir}/.kanban.$$.tmp"

	if [ "$DRY_RUN" -eq 1 ]; then
		detail "[dry-run] would mkdir -p $_dst_dir"
		detail "[dry-run] would atomic copy $_src -> $_dst"
		return 0
	fi

	mkdir -p "$_dst_dir"

	cp "$_src" "$_tmp"
	chmod 755 "$_tmp"
	mv -f "$_tmp" "$_dst"
}

clear_planned_entries() {
	PLANNED_BIN_DST=""
	PLANNED_ENTRIES=""
}

add_planned_entry() {
	_entry="$1"
	if [ -z "$PLANNED_ENTRIES" ]; then
		PLANNED_ENTRIES="$_entry"
	else
		PLANNED_ENTRIES=$(printf '%s\n%s' "$PLANNED_ENTRIES" "$_entry")
	fi
}

discover_skills_dir() {
	if [ -n "${SKILLS_DIR:-}" ]; then
		SKILLS_DIR=$(resolve_path "$SKILLS_DIR")
		log "using skills dir from --skills-dir: $(value "$SKILLS_DIR")"
		return 0
	fi

	if [ "$EXISTING_MANIFEST_HAS_SKILLS" -eq 1 ] && [ -n "$EXISTING_MANIFEST_SKILLS_DIR" ]; then
		SKILLS_DIR="$EXISTING_MANIFEST_SKILLS_DIR"
		log "reusing skills dir from existing install: $(value "$SKILLS_DIR")"
		return 0
	fi

	if [ -n "${OPENCODE_HOME:-}" ]; then
		SKILLS_DIR="${OPENCODE_HOME}/skills"
		log "discovered skills dir from OPENCODE_HOME: $(value "$SKILLS_DIR")"
		return 0
	fi

	if [ -n "${XDG_CONFIG_HOME:-}" ]; then
		SKILLS_DIR="${XDG_CONFIG_HOME}/opencode/skills"
		log "discovered skills dir from XDG_CONFIG_HOME: $(value "$SKILLS_DIR")"
		return 0
	fi

	if [ -d "${HOME}/.config/opencode/skills" ]; then
		SKILLS_DIR="${HOME}/.config/opencode/skills"
		log "discovered existing skills dir: $(value "$SKILLS_DIR")"
		return 0
	fi

	if [ -d "${HOME}/.local/share/opencode/skills" ]; then
		SKILLS_DIR="${HOME}/.local/share/opencode/skills"
		log "discovered existing skills dir: $(value "$SKILLS_DIR")"
		return 0
	fi

	SKILLS_DIR="${HOME}/.config/opencode/skills"
	log "no existing agent config found; proposing default: $(value "$SKILLS_DIR")"
}

path_already_installed() {
	if [ "$EXISTING_MANIFEST_PATH_SELECTED" = "yes" ]; then
		return 0
	fi

	if [ -z "$RC_FILE" ]; then
		return 1
	fi

	_rc_expanded=$(resolve_path "$RC_FILE")
	dir_contains_path "$_rc_expanded" "kanban-installer: PATH"
}

completion_destination() {
	case "$SHELL_NAME" in
		bash)
			if [ -n "${BASH_COMPLETION_USER_DIR:-}" ]; then
				printf '%s/completions/kanban' "$BASH_COMPLETION_USER_DIR"
			else
				printf '%s/.local/share/bash-completion/completions/kanban' "$HOME"
			fi
			;;
		zsh)
			printf '%s/.zsh/completions/_kanban' "$HOME"
			;;
		*)
			printf ''
			;;
	esac
}

completion_already_installed() {
	if [ "$EXISTING_MANIFEST_COMPLETIONS_SELECTED" = "yes" ]; then
		return 0
	fi

	_dst=$(completion_destination)
	if [ -z "$_dst" ]; then
		return 1
	fi

	[ -f "$_dst" ]
}

skills_already_installed() {
	if [ -z "${SKILLS_DIR:-}" ]; then
		return 1
	fi

	if [ "$EXISTING_MANIFEST_HAS_SKILLS" -eq 1 ] && [ "$EXISTING_MANIFEST_SKILLS_DIR" = "$SKILLS_DIR" ]; then
		return 0
	fi

	for _skill in kanban-backlog-maintainer kanban-developer; do
		for _file in SKILL.md plugin.json; do
			if [ -f "${SKILLS_DIR}/${_skill}/${_file}" ]; then
				return 0
			fi
		done
	done

	return 1
}

confirm_or_override() {
	if [ "$SKILLS_DIR_EXPLICIT" -eq 1 ]; then
		SKILLS_DIR=$(resolve_path "$SKILLS_DIR")
		log "agent skills selected: $(value "$SKILLS_DIR") (--skills-dir)"
		return 0
	fi

	if skills_already_installed; then
		SKILLS_DIR=$(resolve_path "$SKILLS_DIR")
		log "agent skills already installed at $(value "$SKILLS_DIR"); updating in place"
		return 0
	fi

	if [ "$YES" -eq 1 ]; then
		SKILLS_DIR=$(resolve_path "$SKILLS_DIR")
		log "agent skills selected: $(value "$SKILLS_DIR") (--yes)"
		return 0
	fi

	if [ "$DRY_RUN" -eq 1 ]; then
		detail "[dry-run] would prompt: Install $(brand) skills to $SKILLS_DIR? [y/N/path]; assuming yes for preview"
		SKILLS_DIR=$(resolve_path "$SKILLS_DIR")
		return 0
	fi

	if ! can_prompt; then
		warn "agent skills skipped (run with --yes or --skills-dir <dir> to install them)"
		SKILLS_DIR=""
		return 0
	fi

	if [ "$QUIET" -eq 0 ]; then
		_checked_open="unset"
		_checked_xdg="unset"
		_checked_config="does not exist"
		_checked_local="does not exist"

		if [ -n "${OPENCODE_HOME:-}" ]; then
			_checked_open="${OPENCODE_HOME}/skills"
		fi
		if [ -n "${XDG_CONFIG_HOME:-}" ]; then
			_checked_xdg="${XDG_CONFIG_HOME}/opencode/skills"
		fi
		if [ -d "${HOME}/.config/opencode/skills" ]; then
			_checked_config="exists"
		fi
		if [ -d "${HOME}/.local/share/opencode/skills" ]; then
			_checked_local="exists"
		fi

		if [ "$_checked_config" = "exists" ] || [ "$_checked_local" = "exists" ] || \
		   [ "$_checked_open" != "unset" ] || [ "$_checked_xdg" != "unset" ]; then
			:
		else
			progress_break_for_prompt
			printf '%b%s%b No existing agent config found.\n' "$C_YELLOW" "$ICON_WARN" "$C_RESET" >&2
			echo "" >&2
			echo "Checked:" >&2
			echo "  - \$OPENCODE_HOME/skills/        ($_checked_open)" >&2
			echo "  - \$XDG_CONFIG_HOME/opencode/skills/  ($_checked_xdg)" >&2
			echo "  - ~/.config/opencode/skills/    ($_checked_config)" >&2
			echo "  - ~/.local/share/opencode/skills/    ($_checked_local)" >&2
			echo "" >&2
		fi
	fi

	progress_break_for_prompt
	printf '%b%s%b Install %s agent skills? %b%s%b [y/N/path] ' "$C_YELLOW" "$ICON_PROMPT" "$C_RESET" "$(brand)" "$C_BOLD" "$SKILLS_DIR" "$C_RESET" > /dev/tty
	read -r _response < /dev/tty

	case "$_response" in
		n|N|no|NO|"")
			log "skill installation declined"
			SKILLS_DIR=""
			return 0
			;;
		y|Y|yes|YES)
			SKILLS_DIR=$(resolve_path "$SKILLS_DIR")
			return 0
			;;
		*)
			_user_path=$(resolve_path "$_response")
			validate_safe_path "$_user_path" "skills-dir"
			SKILLS_DIR="$_user_path"
			log "using user-provided skills dir: $(value "$SKILLS_DIR")"
			return 0
			;;
	esac
}

copy_skill() {
	_skill_name="$1"
	_target_dir="$2"
	_src_dir="${REPO_ROOT}/skills/${_skill_name}"

	if [ ! -d "$_src_dir" ]; then
		if [ "$DRY_RUN" -eq 1 ] && [ "$REMOTE" -eq 1 ]; then
			detail "[dry-run] would install $_skill_name from downloaded release archive"
			return 0
		fi
		die "skill source directory not found: $_src_dir"
	fi

	_dst_dir="${_target_dir}/${_skill_name}"

	for _file in SKILL.md plugin.json; do
		_src="${_src_dir}/${_file}"
		_dst="${_dst_dir}/${_file}"

		if [ ! -f "$_src" ]; then
			die "skill file not found: $_src"
		fi

		_src_hash=$(compute_sha256 "$_src")

		if [ "$DRY_RUN" -eq 1 ]; then
			detail "[dry-run] would mkdir -p $_dst_dir"
			if [ -f "$_dst" ]; then
				_dst_hash=$(compute_sha256 "$_dst" 2>/dev/null || echo "")
				if [ "$_src_hash" != "$_dst_hash" ]; then
					_src_lines=$(wc -l < "$_src" | tr -d ' ')
					_dst_lines=$(wc -l < "$_dst" | tr -d ' ')
					_diff=$((_src_lines - _dst_lines))
					if [ "$_diff" -gt 0 ]; then
						detail "[dry-run] diff summary for $_dst: +${_diff} -0 lines"
					elif [ "$_diff" -lt 0 ]; then
						_diff_abs=$((-_diff))
						detail "[dry-run] diff summary for $_dst: +0 -${_diff_abs} lines"
					fi
				else
					detail "[dry-run] would overwrite $_dst (unchanged)"
				fi
			else
				detail "[dry-run] would cp $_src -> $_dst"
			fi
			continue
		fi

		mkdir -p "$_dst_dir"

		if [ -f "$_dst" ]; then
			_disk_hash=$(compute_sha256 "$_dst" 2>/dev/null || echo "")
			_old_hash=$(lookup_manifest_hash "$_dst")
			if [ -n "$_old_hash" ] && [ "$_old_hash" != "$_disk_hash" ]; then
				if ! prompt_local_edit "$_dst" "$_old_hash" "$_disk_hash"; then
					log "! skipped $_dst (local edits preserved)"
					continue
				fi
			fi
			if [ "$_src_hash" != "$_disk_hash" ]; then
				_src_lines=$(wc -l < "$_src" | tr -d ' ')
				_dst_lines=$(wc -l < "$_dst" | tr -d ' ')
				_diff=$((_src_lines - _dst_lines))
				if [ "$_diff" -gt 0 ]; then
					log "~ diff summary for $_dst: +${_diff} -0 lines"
				elif [ "$_diff" -lt 0 ]; then
					_diff_abs=$((-_diff))
					log "~ diff summary for $_dst: +0 -${_diff_abs} lines"
				fi
			fi
		fi

		atomic_copy "$_src" "$_dst"
		log "+ ${_dst}"
	done
}

do_copy() {
	_src="$1"
	_dst="$2"
	_label="$3"

	_dst_dir=$(dirname "$_dst")
	if [ "$DRY_RUN" -eq 1 ]; then
		detail "[dry-run] would mkdir -p $_dst_dir"
		if [ ! -d "$_dst_dir" ]; then
			:
		fi
		detail "[dry-run] would cp $_src -> $_dst"
		detail "[dry-run] would chmod 755 $_dst"
		return 0
	fi

	mkdir -p "$_dst_dir"
	log "mkdir -p $_dst_dir"

	if [ -f "$_dst" ]; then
		if [ -w "$_dst" ]; then
			:
		else
			die "cannot overwrite $_dst (permission denied)"
		fi
	fi

	cp "$_src" "$_dst"
	chmod 755 "$_dst"
	log "$_label copied to $_dst"
}

do_append_rc() {
	_line="$1"
	_sentinel="$2"

	if [ -z "$RC_FILE" ]; then
		return 0
	fi

	_rc_expanded=$(resolve_path "$RC_FILE")

	if [ "$DRY_RUN" -eq 1 ]; then
		if ! [ -f "$_rc_expanded" ]; then
			detail "[dry-run] would create $_rc_expanded"
		fi
		if dir_contains_path "$_rc_expanded" "$_sentinel"; then
			detail "[dry-run] $_sentinel already present in $_rc_expanded (skip)"
			return 0
		fi
		detail "[dry-run] would append to $_rc_expanded: $_line"
		return 0
	fi

	_rc_dir=$(dirname "$_rc_expanded")
	mkdir -p "$_rc_dir"

	if ! [ -f "$_rc_expanded" ]; then
		touch "$_rc_expanded"
		log "created $_rc_expanded"
	fi

	if dir_contains_path "$_rc_expanded" "$_sentinel"; then
		log "$_sentinel already present in $_rc_expanded (skip)"
		return 0
	fi

	printf '\n%s  # %s\n' "$_line" "$_sentinel" >> "$_rc_expanded"
	log "appended to $_rc_expanded: $_line"
}

do_write_file() {
	_content="$1"
	_dst="$2"
	_label="$3"

	_dst_dir=$(dirname "$_dst")
	if [ "$DRY_RUN" -eq 1 ]; then
		detail "[dry-run] would mkdir -p $_dst_dir"
		detail "[dry-run] would write $_label to $_dst"
		return 0
	fi

	mkdir -p "$_dst_dir"
	log "mkdir -p $_dst_dir"
	printf '%s\n' "$_content" > "$_dst"
	log "wrote $_label to $_dst"
}

do_append_fpath() {
	_fpath_dir="$1"

	if [ -z "$RC_FILE" ] || [ "$SHELL_NAME" != "zsh" ]; then
		return 0
	fi

	_rc_expanded=$(resolve_path "$RC_FILE")

	if dir_contains_path "$_rc_expanded" "fpath=(" 2>/dev/null && \
	   dir_contains_path "$_rc_expanded" "$_fpath_dir" 2>/dev/null; then
		return 0
	fi

	_line="fpath=(\"$_fpath_dir\" \$fpath)"
	_sentinel="kanban-installer: fpath"

	if [ "$DRY_RUN" -eq 1 ]; then
		detail "[dry-run] would append to $_rc_expanded: $_line"
		return 0
	fi

	if dir_contains_path "$_rc_expanded" "$_fpath_dir"; then
		log "fpath entry for $_fpath_dir already present in $_rc_expanded (skip)"
		return 0
	fi

	printf '\n%s  # %s\n' "$_line" "$_sentinel" >> "$_rc_expanded"
	log "appended fpath to $_rc_expanded: $_line"
}

do_ensure_compinit() {
	if [ -z "$RC_FILE" ] || [ "$SHELL_NAME" != "zsh" ]; then
		return 0
	fi

	_rc_expanded=$(resolve_path "$RC_FILE")

	if dir_contains_path "$_rc_expanded" "compinit"; then
		return 0
	fi

	_line="autoload -Uz compinit && compinit"
	_sentinel="kanban-installer: compinit"

	if [ "$DRY_RUN" -eq 1 ]; then
		detail "[dry-run] would append to $_rc_expanded: $_line"
		return 0
	fi

	printf '\n%s  # %s\n' "$_line" "$_sentinel" >> "$_rc_expanded"
	log "appended compinit to $_rc_expanded: $_line"
}

decide_path_update() {
	if [ -z "$RC_FILE" ]; then
		ADD_PATH=0
		log "no known shell rc file detected; PATH profile update unavailable"
		return 0
	fi

	_prefix_resolved=$(resolve_path "$PREFIX")
	_rc_expanded=$(resolve_path "$RC_FILE")

	if [ "${ADD_PATH:-}" = "1" ]; then
		log "PATH profile update selected (--add-path)"
		return 0
	fi
	if [ "${ADD_PATH:-}" = "0" ]; then
		log "PATH profile update skipped (--no-add-path)"
		return 0
	fi
	if path_already_installed; then
		ADD_PATH=1
		log "PATH profile update already installed in $(value "$_rc_expanded"); updating in place"
		return 0
	fi
	if [ "$YES" -eq 1 ]; then
		ADD_PATH=1
		log "PATH profile update selected: $(value "$_rc_expanded") (--yes)"
		return 0
	fi
	if [ "$DRY_RUN" -eq 1 ]; then
		detail "[dry-run] would prompt: Add $_prefix_resolved to PATH in $_rc_expanded? [y/N]; assuming yes for preview"
		ADD_PATH=1
		return 0
	fi
	if prompt_yes_no "Add $_prefix_resolved to PATH in $_rc_expanded?" "no"; then
		ADD_PATH=1
		return 0
	fi

	ADD_PATH=0
	warn "PATH profile update skipped (run with --add-path to enable it)"
}

decide_completion_install() {
	case "$SHELL_NAME" in
		bash|zsh) : ;;
		"")
			COMPLETIONS=0
			log "completion installation skipped (unknown shell)"
			return 0
			;;
		*)
			COMPLETIONS=0
			log "completion installation skipped for $(value "$SHELL_NAME") (unsupported shell)"
			return 0
			;;
	esac

	if [ "${COMPLETIONS:-}" = "1" ]; then
		log "shell completion installation selected (--completions)"
		return 0
	fi
	if [ "${COMPLETIONS:-}" = "0" ]; then
		log "shell completion installation skipped (--no-completions)"
		return 0
	fi
	if completion_already_installed; then
		COMPLETIONS=1
		log "shell completion already installed for $(value "$SHELL_NAME"); updating in place"
		return 0
	fi
	if [ "$YES" -eq 1 ]; then
		COMPLETIONS=1
		log "shell completion installation selected for $(value "$SHELL_NAME") (--yes)"
		return 0
	fi
	if [ "$DRY_RUN" -eq 1 ]; then
		detail "[dry-run] would prompt: Install $SHELL_NAME shell completions? [y/N]; assuming yes for preview"
		COMPLETIONS=1
		return 0
	fi
	if prompt_yes_no "Install $SHELL_NAME shell completions?" "no"; then
		COMPLETIONS=1
		return 0
	fi

	COMPLETIONS=0
	warn "shell completion installation skipped (run with --completions to enable it)"
}

install_binary() {
	_prefix_resolved=$(resolve_path "$PREFIX")
	_dst="${_prefix_resolved}/${BINARY_NAME}"

	log "installing $(brand) binary"
	atomic_copy "$BINARY" "$_dst"
	log "$(brand) binary copied to $(value "$_dst")"
}

append_path_export() {
	if [ "${ADD_PATH:-0}" -ne 1 ]; then
		return 0
	fi

	if [ -z "$RC_FILE" ]; then
		log "no known shell rc file detected; PATH bootstrap skipped"
		return 0
	fi

	_prefix_resolved=$(resolve_path "$PREFIX")
	_escaped_prefix=$(echo "$_prefix_resolved" | sed 's/\\/\\\\/g')
	_line="export PATH=\"$_escaped_prefix:\$PATH\""
	_sentinel="kanban-installer: PATH"

	do_append_rc "$_line" "$_sentinel"
}

install_completion() {
	if [ "${COMPLETIONS:-0}" -ne 1 ]; then
		return 0
	fi

	if [ -z "$SHELL_NAME" ]; then
		return 0
	fi

	case "$SHELL_NAME" in
		bash) install_bash_completion ;;
		zsh)  install_zsh_completion ;;
		*)
			log "completion installation skipped for $(value "$SHELL_NAME") (unsupported shell)"
			;;
	esac
}

install_bash_completion() {
	_dst=$(completion_destination)

	if [ "$DRY_RUN" -eq 1 ]; then
		if [ -x "$PREFIX/$BINARY_NAME" ]; then
			_completion=$("$PREFIX/$BINARY_NAME" completion bash 2>/dev/null || true)
		elif [ -x "$BINARY" ]; then
			_completion=$("$BINARY" completion bash 2>/dev/null || true)
		else
			_completion="[would generate completion]"
		fi
		do_write_file "$_completion" "$_dst" "bash completion"
		return 0
	fi

	_cmd="$PREFIX/$BINARY_NAME"
	if [ -x "$_cmd" ]; then
		:
	elif [ -x "$BINARY" ]; then
		_cmd="$BINARY"
	else
		die "$(brand) binary not executable; cannot generate completion"
	fi

	_completion=$("$_cmd" completion bash 2>/dev/null) || die "$(brand) completion bash failed"
	do_write_file "$_completion" "$_dst" "bash completion"
}

install_zsh_completion() {
	_zsh_comp_dir="${HOME}/.zsh/completions"
	_dst=$(completion_destination)

	if [ "$DRY_RUN" -eq 1 ]; then
		if [ -x "$PREFIX/$BINARY_NAME" ]; then
			_completion=$("$PREFIX/$BINARY_NAME" completion zsh 2>/dev/null || true)
		elif [ -x "$BINARY" ]; then
			_completion=$("$BINARY" completion zsh 2>/dev/null || true)
		else
			_completion="[would generate completion]"
		fi
		detail "[dry-run] would write zsh completion to $_dst"
		do_append_fpath "$_zsh_comp_dir"
		do_ensure_compinit
		return 0
	fi

	_cmd="$PREFIX/$BINARY_NAME"
	if [ -x "$_cmd" ]; then
		:
	elif [ -x "$BINARY" ]; then
		_cmd="$BINARY"
	else
		die "$(brand) binary not executable; cannot generate completion"
	fi

	_completion=$("$_cmd" completion zsh 2>/dev/null) || die "$(brand) completion zsh failed"

	do_write_file "$_completion" "$_dst" "zsh completion"
	do_append_fpath "$_zsh_comp_dir"
	do_ensure_compinit
}

install_skills() {
	if [ "$NO_SKILLS" -eq 1 ]; then
		log "skill installation skipped (--no-skills)"
		return 0
	fi

	if [ -z "${SKILLS_DIR:-}" ]; then
		log "no skills directory selected; skipping skill install"
		return 0
	fi

	copy_skill "kanban-backlog-maintainer" "$SKILLS_DIR"
	copy_skill "kanban-developer" "$SKILLS_DIR"

	log "skills installed to $(value "$SKILLS_DIR")"
}

gather_planned_files() {
	clear_planned_entries

	_prefix_resolved=$(resolve_path "$PREFIX")
	_bin_dst="${_prefix_resolved}/${BINARY_NAME}"
	PLANNED_BIN_DST="$_bin_dst"

	if [ -f "$BINARY" ]; then
		_bin_hash=$(compute_sha256 "$BINARY")
	else
		_bin_hash="dry-run"
	fi
	add_planned_entry "$(printf '%s\t%s\t%s\t%s' "$_bin_dst" "$_bin_hash" "$BINARY" "$INSTALLER_VERSION")"

	if [ "${COMPLETIONS:-0}" -eq 1 ] && [ -n "$SHELL_NAME" ]; then
		_dst=$(completion_destination)
		if [ -n "$_dst" ]; then
			if [ -x "$BINARY" ]; then
				if [ "$SHELL_NAME" = "bash" ]; then
					_completion=$("$BINARY" completion bash 2>/dev/null || echo "")
				elif [ "$SHELL_NAME" = "zsh" ]; then
					_completion=$("$BINARY" completion zsh 2>/dev/null || echo "")
				else
					_completion=""
				fi
				if [ -n "$_completion" ]; then
					_completion_hash=$(printf '%s' "$_completion" | $(find_sha256_cmd) | awk '{print $1}')
					add_planned_entry "$(printf '%s\t%s\tgenerated:kanban-completion-%s\t%s' "$_dst" "$_completion_hash" "$SHELL_NAME" "$INSTALLER_VERSION")"
				fi
			fi
		fi
	fi

	if [ "$NO_SKILLS" -eq 0 ] && [ -n "${SKILLS_DIR:-}" ]; then
		for _skill in kanban-backlog-maintainer kanban-developer; do
			_src_dir="${REPO_ROOT}/skills/${_skill}"
			[ -d "$_src_dir" ] || continue
			for _file in SKILL.md plugin.json; do
				_src="${_src_dir}/${_file}"
				[ -f "$_src" ] || continue
				_dst="${SKILLS_DIR}/${_skill}/${_file}"
				_src_hash=$(compute_sha256 "$_src")
				add_planned_entry "$(printf '%s\t%s\trepo:skills/%s/%s\t%s' "$_dst" "$_src_hash" "$_skill" "$_file" "$INSTALLER_VERSION")"
			done
		done
	fi
}

write_manifest() {
	_manifest_dir=$(manifest_dir)
	_manifest=$(manifest_path)
	_manifest_tmp="${_manifest_dir}/.manifest.$$.tmp"
	_installed_at=$(iso8601_now)
	_path_installed="no"
	_completions_installed="no"
	if [ "${ADD_PATH:-0}" -eq 1 ]; then
		_path_installed="yes"
	fi
	if [ "${COMPLETIONS:-0}" -eq 1 ]; then
		_completions_installed="yes"
	fi

	if [ "$DRY_RUN" -eq 1 ]; then
		detail "[dry-run] would write manifest to $_manifest"
		return 0
	fi

	mkdir -p "$_manifest_dir"

	{
		printf '# kanban install manifest\n'
		printf '# installer-version: %s\n' "$INSTALLER_VERSION"
		printf '# installed-at: %s\n' "$_installed_at"
		printf '# prefix: %s\n' "$(resolve_path "$PREFIX")"
		printf '# path-installed: %s\n' "$_path_installed"
		printf '# completions-installed: %s\n' "$_completions_installed"
		if [ -n "${SKILLS_DIR:-}" ]; then
			printf '# skills-dir: %s\n' "$SKILLS_DIR"
		fi
		printf '\n'
		if [ -n "$PLANNED_ENTRIES" ]; then
			printf '%s\n' "$PLANNED_ENTRIES"
		fi
	} > "$_manifest_tmp"
	mv -f "$_manifest_tmp" "$_manifest"

	log "wrote manifest to $(value "$_manifest")"
}

main() {
	parse_args "$@"
	init_ui
	init_log_file

	if [ -n "$BINARY" ]; then
		case "$BINARY" in
			*.exe) BINARY_NAME="kanban.exe" ;;
		esac
		log "source binary: $(value "$BINARY")"
	fi
	log "install prefix: $(value "$PREFIX")"
	if [ "$DRY_RUN" -eq 1 ]; then
		log "mode: dry-run (no filesystem changes)"
	fi

	UMASK_SAVED=$(umask)
	umask 022

	find_repo_root
	detect_shell
	load_existing_manifest_state

	if [ "$REMOTE" -eq 1 ]; then
		log "remote install mode"
		TARGET_TRIPLE=$(detect_target) || die "unsupported target: $(uname -s) $(uname -m)"
		case "$TARGET_TRIPLE" in
			*-windows-*) BINARY_NAME="kanban.exe" ;;
			*) BINARY_NAME="kanban" ;;
		esac
		log "detected target: $(value "$TARGET_TRIPLE")"
		resolve_remote_source
		fetch_and_verify
		BINARY="${EXTRACTED_DIR}/${BINARY_NAME}"
		REPO_ROOT="$EXTRACTED_DIR"
		log "using binary: $(value "$BINARY")"
	fi

	progress_start "installing $(brand) $(version "$(display_install_version)")"

	if [ -n "$RC_FILE" ]; then
		log "detected shell: $(value "$SHELL_NAME") (rc: $(value "$RC_FILE"))"
	else
		log "detected shell: $(value "$SHELL_NAME") (no known rc file)"
	fi
	progress_step "environment detected"

	if [ ! -f "$BINARY" ] && [ "$DRY_RUN" -eq 1 ] && [ "$REMOTE" -eq 1 ]; then
		log "dry-run: remote binary would be downloaded, skipping existence check"
	elif [ ! -f "$BINARY" ]; then
		die "binary not found at $BINARY"
	fi

	if [ ! -x "$BINARY" ] && [ "$DRY_RUN" -eq 0 ]; then
		log "warning: $(value "$BINARY") is not executable; continuing anyway"
	fi

	decide_path_update
	decide_completion_install
	if [ "$NO_SKILLS" -eq 0 ]; then
		discover_skills_dir
		if [ -n "${SKILLS_DIR:-}" ]; then
			validate_safe_path "$SKILLS_DIR" "skills-dir"
			confirm_or_override
		fi
	fi

	gather_planned_files
	progress_step "install plan prepared"

	_manifest=$(manifest_path)
	if [ -f "$_manifest" ]; then
		_old_prefix=$(grep '^# prefix:' "$_manifest" 2>/dev/null | sed 's/^# prefix: //' || echo "")
		_current_prefix=$(resolve_path "$PREFIX")
		if [ -n "$_old_prefix" ] && [ "$_old_prefix" != "$_current_prefix" ]; then
			log "! different --prefix detected: old=$(value "$_old_prefix") new=$(value "$_current_prefix")"
			log "! treating as fresh install; previous install at $(value "$_old_prefix") left untouched"
			if [ -n "${SKILLS_DIR:-}" ]; then
				_old_skills=$(grep '^# skills-dir:' "$_manifest" 2>/dev/null | sed 's/^# skills-dir: //' || echo "")
				if [ -n "$_old_skills" ] && [ "$_old_skills" != "$SKILLS_DIR" ]; then
					log "! different --skills-dir detected: old=$(value "$_old_skills") new=$(value "$SKILLS_DIR")"
					log "! previous skills at $(value "$_old_skills") left untouched"
					log "! run scripts/uninstall.sh --prefix $(value "$(dirname "$_old_prefix")") --skills-dir $(value "$_old_skills") to clean up"
				fi
			fi
		else
			reconcile_manifest "$_manifest"
			_old_bin_ver=$(get_binary_version "$PLANNED_BIN_DST")
			_new_bin_ver=$(get_binary_version "$BINARY")
			prompt_downgrade "$_old_bin_ver" "$_new_bin_ver"
			remove_orphans
		fi
	else
		log "no previous manifest found; fresh install"
	fi
	progress_step "previous install reconciled"

	install_binary
	progress_step "binary installed"
	append_path_export
	install_completion
	progress_step "shell integrations handled"
	install_skills
	progress_step "agent skills handled"
	write_manifest
	progress_step "manifest written"

	if [ "$DRY_RUN" -eq 1 ]; then
		progress_finish "dry-run complete — no changes were made."
	else
		progress_finish "install complete."
	fi
}

main "$@"
