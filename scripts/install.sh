#!/bin/sh
set -eu

INSTALLER_VERSION="26.6.2115"
UMASK_SAVED=""
RC_FILE=""
SHELL_NAME=""

BINARY=""
PREFIX=""
DRY_RUN=0
QUIET=0
SKILLS_DIR=""
NO_SKILLS=0
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

cleanup() {
	if [ -n "${UMASK_SAVED:-}" ]; then
		umask "$UMASK_SAVED" 2>/dev/null || true
	fi
}

trap cleanup EXIT

log() {
	if [ "$QUIET" -eq 0 ]; then
		echo "+ $*" >&2
	fi
}

die() {
	echo "kanban-installer: $*" >&2
	exit 1
}

usage() {
	cat >&2 <<'USAGE'
Usage: sh scripts/install.sh --binary <path> [flags]

Flags:
  --binary <path>   Path to the prebuilt kanban binary (required)
  --prefix <dir>    Install directory for the binary (default: ~/.local/bin)
  --skills-dir <dir>  Install agent skills to <dir> (skips discovery and prompt)
  --no-skills       Skip agent skill installation entirely
  --yes             Accept all defaults without prompting
  --force           Skip safety prompts (downgrade, local-edit detection)
  --dry-run         Preview all actions without modifying the filesystem
  --version <tag>   Install a specific release from remote (e.g. --version v26.6.2114)
  --channel main    Install from main/nightly channel (warned)
  --offline         Use cached artifacts only, no network
  --cache-dir <dir> Override download cache dir (default: ~/.cache/kanban)
  --quiet           Suppress non-error log lines

Local install (requires --binary):
  sh scripts/install.sh --binary ./target/release/kanban

Remote install:
  curl -fsSL <url>/scripts/install.sh | sh -s -- --version v26.6.2114
  sh scripts/install.sh --version v26.6.2114
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
				shift 2
				;;
			--no-skills)
				NO_SKILLS=1
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
		usage
		exit 2
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
		*) return 1 ;;
	esac
}

resolve_remote_source() {
	if [ -z "${GITHUB_REPO_BASE:-}" ]; then
		RELEASE_BASE="https://github.com/anomalyco/autopass-kanban/releases/download"
	else
		RELEASE_BASE="$GITHUB_REPO_BASE"
	fi

	if [ -n "${REMOTE_VERSION:-}" ]; then
		_ver="${REMOTE_VERSION#v}"
		TARBALL_URL="${RELEASE_BASE}/v${_ver}/kanban-${_ver}-${TARGET_TRIPLE}.tar.gz"
		CHECKSUMS_URL="${RELEASE_BASE}/v${_ver}/kanban-${_ver}-checksums.txt"
		REMOTE_ARTIFACT_VERSION="$_ver"
		return 0
	fi

	if [ -n "${REMOTE_CHANNEL:-}" ]; then
		die "remote channel 'main' requires a published release; use --version instead"
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
		echo "[dry-run] would download $_tarball_name from $RELEASE_BASE" >&2
		echo "[dry-run] would verify sha256 against $_checksums_name" >&2
		EXTRACTED_DIR="${_cache}/kanban-${REMOTE_ARTIFACT_VERSION}-${TARGET_TRIPLE}"
		return 0
	fi

	mkdir -p "$_cache"

	if [ "$OFFLINE" -eq 0 ]; then
		if [ ! -f "$_tarball_cache" ] || [ ! -f "$_checksums_cache" ]; then
			log "downloading $TARBALL_URL"
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
	log "checksum verified: $_tarball_name"

	EXTRACTED_DIR="${_cache}/kanban-${REMOTE_ARTIFACT_VERSION}-${TARGET_TRIPLE}"
	rm -rf "$EXTRACTED_DIR"
	mkdir -p "$EXTRACTED_DIR"
	tar -xzf "$_tarball_cache" -C "$EXTRACTED_DIR"
	log "extracted to $EXTRACTED_DIR"
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
		if [ "$QUIET" -eq 0 ]; then
			echo "! no previous manifest found; treating as fresh install" >&2
		fi
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
			echo "[dry-run] would remove orphan: $_path" >&2
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

	if [ "$YES" -eq 1 ] || [ ! -t 0 ]; then
		log "downgrade refused ($_old_ver -> $_new_ver); use --force to override"
		die "downgrade refused (old=$_old_ver new=$_new_ver)"
	fi

	if [ "$DRY_RUN" -eq 1 ]; then
		echo "[dry-run] would prompt: Downgrade from $_old_ver to $_new_ver? [y/N]" >&2
		return 0
	fi

	printf 'Downgrade from %s to %s? [y/N] ' "$_old_ver" "$_new_ver" > /dev/tty
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

	if [ "$YES" -eq 1 ] || [ ! -t 0 ]; then
		log "skipping locally edited file: $_path (use --force to overwrite)"
		return 1
	fi

	if [ "$DRY_RUN" -eq 1 ]; then
		echo "[dry-run] would prompt: Local edits to $_path will be overwritten. Continue? [y/N]" >&2
		return 0
	fi

	if [ "$QUIET" -eq 0 ]; then
		echo "! local edits detected in $_path (hash changed from manifest)" >&2
	fi
	printf 'Local edits to %s will be overwritten. Continue? [y/N] ' "$_path" > /dev/tty
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
		echo "[dry-run] would mkdir -p $_dst_dir" >&2
		echo "[dry-run] would atomic copy $_src -> $_dst" >&2
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
		log "using skills dir from --skills-dir: $SKILLS_DIR"
		return 0
	fi

	if [ -n "${OPENCODE_HOME:-}" ]; then
		SKILLS_DIR="${OPENCODE_HOME}/skills"
		log "discovered skills dir from OPENCODE_HOME: $SKILLS_DIR"
		return 0
	fi

	if [ -n "${XDG_CONFIG_HOME:-}" ]; then
		SKILLS_DIR="${XDG_CONFIG_HOME}/opencode/skills"
		log "discovered skills dir from XDG_CONFIG_HOME: $SKILLS_DIR"
		return 0
	fi

	if [ -d "${HOME}/.config/opencode/skills" ]; then
		SKILLS_DIR="${HOME}/.config/opencode/skills"
		log "discovered existing skills dir: $SKILLS_DIR"
		return 0
	fi

	if [ -d "${HOME}/.local/share/opencode/skills" ]; then
		SKILLS_DIR="${HOME}/.local/share/opencode/skills"
		log "discovered existing skills dir: $SKILLS_DIR"
		return 0
	fi

	SKILLS_DIR="${HOME}/.config/opencode/skills"
	log "no existing agent config found; proposing default: $SKILLS_DIR"
}

confirm_or_override() {
	if [ "$YES" -eq 1 ]; then
		log "auto-accepting skills dir: $SKILLS_DIR (--yes)"
		return 0
	fi

	if [ ! -t 0 ]; then
		log "non-interactive: defaulting skills dir to $SKILLS_DIR"
		return 0
	fi

	if [ "$DRY_RUN" -eq 1 ]; then
		echo "[dry-run] would prompt: Install kanban skills to $SKILLS_DIR/? [Y/n/path]" >&2
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
			echo "kanban installer: no existing agent config found." >&2
			echo "" >&2
			echo "Checked:" >&2
			echo "  - \$OPENCODE_HOME/skills/        ($_checked_open)" >&2
			echo "  - \$XDG_CONFIG_HOME/opencode/skills/  ($_checked_xdg)" >&2
			echo "  - ~/.config/opencode/skills/    ($_checked_config)" >&2
			echo "  - ~/.local/share/opencode/skills/    ($_checked_local)" >&2
			echo "" >&2
		fi
	fi

	printf 'Install kanban skills to %s/? [Y/n/path] ' "$SKILLS_DIR" > /dev/tty
	read -r _response < /dev/tty

	case "$_response" in
		n|N)
			printf 'Enter alternative path (or leave empty to skip): ' > /dev/tty
			read -r _response < /dev/tty
			if [ -n "$_response" ]; then
				_user_path=$(resolve_path "$_response")
				validate_safe_path "$_user_path" "skills-dir"
				SKILLS_DIR="$_user_path"
				log "using user-provided skills dir: $SKILLS_DIR"
				return 0
			fi
			log "skill installation declined"
			SKILLS_DIR=""
			return 0
			;;
		y|Y|"")
			SKILLS_DIR=$(resolve_path "$SKILLS_DIR")
			return 0
			;;
		*)
			_user_path=$(resolve_path "$_response")
			validate_safe_path "$_user_path" "skills-dir"
			SKILLS_DIR="$_user_path"
			log "using user-provided skills dir: $SKILLS_DIR"
			return 0
			;;
	esac
}

copy_skill() {
	_skill_name="$1"
	_target_dir="$2"
	_src_dir="${REPO_ROOT}/skills/${_skill_name}"

	if [ ! -d "$_src_dir" ]; then
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
			echo "[dry-run] would mkdir -p $_dst_dir" >&2
			if [ -f "$_dst" ]; then
				_dst_hash=$(compute_sha256 "$_dst" 2>/dev/null || echo "")
				if [ "$_src_hash" != "$_dst_hash" ]; then
					_src_lines=$(wc -l < "$_src" | tr -d ' ')
					_dst_lines=$(wc -l < "$_dst" | tr -d ' ')
					_diff=$((_src_lines - _dst_lines))
					if [ "$_diff" -gt 0 ]; then
						echo "[dry-run] diff summary for $_dst: +${_diff} -0 lines" >&2
					elif [ "$_diff" -lt 0 ]; then
						_diff_abs=$((-_diff))
						echo "[dry-run] diff summary for $_dst: +0 -${_diff_abs} lines" >&2
					fi
				else
					echo "[dry-run] would overwrite $_dst (unchanged)" >&2
				fi
			else
				echo "[dry-run] would cp $_src -> $_dst" >&2
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
		echo "[dry-run] would mkdir -p $_dst_dir" >&2
		if [ ! -d "$_dst_dir" ]; then
			:
		fi
		echo "[dry-run] would cp $_src -> $_dst" >&2
		echo "[dry-run] would chmod 755 $_dst" >&2
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
			echo "[dry-run] would create $_rc_expanded" >&2
		fi
		if dir_contains_path "$_rc_expanded" "$_sentinel"; then
			echo "[dry-run] $_sentinel already present in $_rc_expanded (skip)" >&2
			return 0
		fi
		echo "[dry-run] would append to $_rc_expanded: $_line" >&2
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
		echo "[dry-run] would mkdir -p $_dst_dir" >&2
		echo "[dry-run] would write $_label to $_dst" >&2
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
		echo "[dry-run] would append to $_rc_expanded: $_line" >&2
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
		echo "[dry-run] would append to $_rc_expanded: $_line" >&2
		return 0
	fi

	printf '\n%s  # %s\n' "$_line" "$_sentinel" >> "$_rc_expanded"
	log "appended compinit to $_rc_expanded: $_line"
}

install_binary() {
	_prefix_resolved=$(resolve_path "$PREFIX")
	_dst="${_prefix_resolved}/kanban"

	log "installing kanban binary"
	atomic_copy "$BINARY" "$_dst"
	log "kanban binary copied to $_dst"
}

append_path_export() {
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
	if [ -z "$SHELL_NAME" ]; then
		return 0
	fi

	case "$SHELL_NAME" in
		bash) install_bash_completion ;;
		zsh)  install_zsh_completion ;;
		*)
			log "completion installation skipped for $SHELL_NAME (unsupported shell)"
			;;
	esac
}

install_bash_completion() {
	if [ -n "${BASH_COMPLETION_USER_DIR:-}" ]; then
		_bash_comp_dir="${BASH_COMPLETION_USER_DIR}/completions"
	else
		_bash_comp_dir="${HOME}/.local/share/bash-completion/completions"
	fi

	_dst="${_bash_comp_dir}/kanban"

	if [ "$DRY_RUN" -eq 1 ]; then
		if [ -x "$PREFIX/kanban" ]; then
			_completion=$("$PREFIX/kanban" completion bash 2>/dev/null || true)
		elif [ -x "$BINARY" ]; then
			_completion=$("$BINARY" completion bash 2>/dev/null || true)
		else
			_completion="[would generate completion]"
		fi
		do_write_file "$_completion" "$_dst" "bash completion"
		return 0
	fi

	_cmd="$PREFIX/kanban"
	if [ -x "$_cmd" ]; then
		:
	elif [ -x "$BINARY" ]; then
		_cmd="$BINARY"
	else
		die "kanban binary not executable; cannot generate completion"
	fi

	_completion=$("$_cmd" completion bash 2>/dev/null) || die "kanban completion bash failed"
	do_write_file "$_completion" "$_dst" "bash completion"
}

install_zsh_completion() {
	_zsh_comp_dir="${HOME}/.zsh/completions"
	_dst="${_zsh_comp_dir}/_kanban"

	if [ "$DRY_RUN" -eq 1 ]; then
		if [ -x "$PREFIX/kanban" ]; then
			_completion=$("$PREFIX/kanban" completion zsh 2>/dev/null || true)
		elif [ -x "$BINARY" ]; then
			_completion=$("$BINARY" completion zsh 2>/dev/null || true)
		else
			_completion="[would generate completion]"
		fi
		echo "[dry-run] would write zsh completion to $_dst" >&2
		do_append_fpath "$_zsh_comp_dir"
		do_ensure_compinit
		return 0
	fi

	_cmd="$PREFIX/kanban"
	if [ -x "$_cmd" ]; then
		:
	elif [ -x "$BINARY" ]; then
		_cmd="$BINARY"
	else
		die "kanban binary not executable; cannot generate completion"
	fi

	_completion=$("$_cmd" completion zsh 2>/dev/null) || die "kanban completion zsh failed"

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

	log "skills installed to $SKILLS_DIR"
}

gather_planned_files() {
	clear_planned_entries

	_prefix_resolved=$(resolve_path "$PREFIX")
	_bin_dst="${_prefix_resolved}/kanban"
	PLANNED_BIN_DST="$_bin_dst"

	_bin_hash=$(compute_sha256 "$BINARY")
	add_planned_entry "$(printf '%s\t%s\t%s\t%s' "$_bin_dst" "$_bin_hash" "$BINARY" "$INSTALLER_VERSION")"

	if [ -n "$SHELL_NAME" ]; then
		case "$SHELL_NAME" in
			bash)
				if [ -n "${BASH_COMPLETION_USER_DIR:-}" ]; then
					_bash_comp_dir="${BASH_COMPLETION_USER_DIR}/completions"
				else
					_bash_comp_dir="${HOME}/.local/share/bash-completion/completions"
				fi
				_dst="${_bash_comp_dir}/kanban"
				;;
			zsh)
				_zsh_comp_dir="${HOME}/.zsh/completions"
				_dst="${_zsh_comp_dir}/_kanban"
				;;
			*)
				_dst=""
				;;
		esac
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

	if [ "$DRY_RUN" -eq 1 ]; then
		echo "[dry-run] would write manifest to $_manifest" >&2
		return 0
	fi

	mkdir -p "$_manifest_dir"

	{
		printf '# kanban install manifest\n'
		printf '# installer-version: %s\n' "$INSTALLER_VERSION"
		printf '# installed-at: %s\n' "$_installed_at"
		printf '# prefix: %s\n' "$(resolve_path "$PREFIX")"
		if [ -n "${SKILLS_DIR:-}" ]; then
			printf '# skills-dir: %s\n' "$SKILLS_DIR"
		fi
		printf '\n'
		if [ -n "$PLANNED_ENTRIES" ]; then
			printf '%s\n' "$PLANNED_ENTRIES"
		fi
	} > "$_manifest_tmp"
	mv -f "$_manifest_tmp" "$_manifest"

	log "wrote manifest to $_manifest"
}

main() {
	parse_args "$@"

	log "kanban-installer v${INSTALLER_VERSION}"
	log "source binary: $BINARY"
	log "install prefix: $PREFIX"
	if [ "$DRY_RUN" -eq 1 ]; then
		log "mode: dry-run (no filesystem changes)"
	fi

	UMASK_SAVED=$(umask)
	umask 022

	find_repo_root
	detect_shell

	if [ "$REMOTE" -eq 1 ]; then
		log "remote install mode"
		TARGET_TRIPLE=$(detect_target) || die "unsupported target: $(uname -s) $(uname -m)"
		log "detected target: $TARGET_TRIPLE"
		resolve_remote_source
		fetch_and_verify
		if [ "$DRY_RUN" -eq 0 ]; then
			BINARY="${EXTRACTED_DIR}/kanban"
			REPO_ROOT="$EXTRACTED_DIR"
		fi
		log "using binary: $BINARY"
	fi

	if [ -n "$RC_FILE" ]; then
		log "detected shell: $SHELL_NAME (rc: $RC_FILE)"
	else
		log "detected shell: $SHELL_NAME (no known rc file)"
	fi

	if [ ! -f "$BINARY" ] && [ "$DRY_RUN" -eq 1 ] && [ "$REMOTE" -eq 1 ]; then
		log "dry-run: remote binary would be downloaded, skipping existence check"
	elif [ ! -f "$BINARY" ]; then
		die "binary not found at $BINARY"
	fi

	if [ ! -x "$BINARY" ] && [ "$DRY_RUN" -eq 0 ]; then
		log "warning: $BINARY is not executable; continuing anyway"
	fi

	if [ "$NO_SKILLS" -eq 0 ]; then
		discover_skills_dir
		if [ -n "${SKILLS_DIR:-}" ]; then
			validate_safe_path "$SKILLS_DIR" "skills-dir"
			confirm_or_override
		fi
	fi

	gather_planned_files

	_manifest=$(manifest_path)
	if [ -f "$_manifest" ]; then
		_old_prefix=$(grep '^# prefix:' "$_manifest" 2>/dev/null | sed 's/^# prefix: //' || echo "")
		_current_prefix=$(resolve_path "$PREFIX")
		if [ -n "$_old_prefix" ] && [ "$_old_prefix" != "$_current_prefix" ]; then
			log "! different --prefix detected: old=$_old_prefix new=$_current_prefix"
			log "! treating as fresh install; previous install at $_old_prefix left untouched"
			if [ -n "${SKILLS_DIR:-}" ]; then
				_old_skills=$(grep '^# skills-dir:' "$_manifest" 2>/dev/null | sed 's/^# skills-dir: //' || echo "")
				if [ -n "$_old_skills" ] && [ "$_old_skills" != "$SKILLS_DIR" ]; then
					log "! different --skills-dir detected: old=$_old_skills new=$SKILLS_DIR"
					log "! previous skills at $_old_skills left untouched"
					log "! run scripts/uninstall.sh --prefix $(dirname "$_old_prefix") --skills-dir $_old_skills to clean up"
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

	install_binary
	append_path_export
	install_completion
	install_skills
	write_manifest

	if [ "$DRY_RUN" -eq 1 ]; then
		log "dry-run complete — no changes were made"
	else
		log "install complete"
	fi
}

main "$@"
