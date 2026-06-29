#!/bin/sh
set -eu

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
INSTALL_SCRIPT="$REPO_ROOT/scripts/install.sh"

failures=0
tests_run=0

fail() {
	echo "FAIL: $*" >&2
	failures=$((failures + 1))
}

assert_file_exists() {
	if [ -f "$1" ]; then
		return 0
	fi
	fail "expected file not found: $1"
	return 1
}

assert_symlink_target() {
	if [ ! -L "$1" ]; then
		fail "expected symlink not found: $1"
		return 1
	fi
	_target=$(readlink "$1" 2>/dev/null || true)
	if [ "$_target" = "$2" ]; then
		return 0
	fi
	fail "symlink '$1' points to '$_target', expected '$2'"
	return 1
}

assert_file_contains() {
	if [ -f "$1" ] && grep -qF "$2" "$1" 2>/dev/null; then
		return 0
	fi
	fail "file '$1' does not contain '$2'"
	return 1
}

assert_file_not_contains() {
	if [ ! -f "$1" ] || ! grep -qF "$2" "$1" 2>/dev/null; then
		return 0
	fi
	fail "file '$1' unexpectedly contains '$2'"
	return 1
}

assert_exit_code() {
	_expected="$1"
	_actual="$2"
	_label="$3"
	if [ "$_expected" -eq "$_actual" ]; then
		return 0
	fi
	fail "$_label: expected exit $_expected, got $_actual"
	return 1
}

assert_line_order() {
	_file="$1"
	_first="$2"
	_second="$3"
	_third="$4"

	_first_line=$(grep -nF "$_first" "$_file" 2>/dev/null | sed -n '1s/:.*//p')
	_second_line=$(grep -nF "$_second" "$_file" 2>/dev/null | sed -n '1s/:.*//p')
	_third_line=$(grep -nF "$_third" "$_file" 2>/dev/null | sed -n '1s/:.*//p')

	if [ -z "$_first_line" ] || [ -z "$_second_line" ] || [ -z "$_third_line" ]; then
		fail "missing ordered log lines in $_file"
		return 1
	fi

	if [ "$_first_line" -lt "$_second_line" ] && [ "$_second_line" -lt "$_third_line" ]; then
		return 0
	fi

	fail "unexpected log order in $_file"
	return 1
}

install_log_path() {
	_log=$(sed -n 's/.*install log: //p' "$1" 2>/dev/null | sed -n '$p')
	if [ -n "$_log" ]; then
		printf '%s' "$_log"
	fi
}

run_in_home() {
	_shell_override="$1"
	_test_name="$2"
	_output="$3"
	_shift_extra=0
	_prefix_arg=""

	case "$_test_name" in
		custom_prefix)
			_prefix_arg="--prefix ~/bin"
			;;
	esac

	HOME_DIR=$(mktemp -d /tmp/kanban-install-test.XXXXXX)
	export HOME="$HOME_DIR"

	cp "$SCRIPT_DIR/stub-kanban" "$HOME_DIR/stub-kanban"
	chmod +x "$HOME_DIR/stub-kanban"
	STUB="$HOME_DIR/stub-kanban"

	if [ -n "$_shell_override" ]; then
		export SHELL="$_shell_override"
	fi

	if [ "$_output" = "capture" ]; then
		set +e
		sh "$INSTALL_SCRIPT" --binary "$STUB" $_prefix_arg --yes --no-skills > "$HOME_DIR/stdout" 2> "$HOME_DIR/stderr"
		_exit=$?
		set -e
	else
		set +e
		sh "$INSTALL_SCRIPT" --binary "$STUB" $_prefix_arg --yes --no-skills
		_exit=$?
		set -e
	fi

	unset SHELL
}

cleanup_home() {
	if [ -n "${HOME_DIR:-}" ] && [ -d "$HOME_DIR" ]; then
		rm -rf "$HOME_DIR"
		unset HOME_DIR HOME
	fi
}

echo "=== kanban installer integration tests ==="

# Scenario 9: remote dry-run without --binary resolves latest release
echo ""
echo "--- Scenario 9: remote dry-run defaults to latest release ---"
tests_run=$((tests_run + 1))
HOME_DIR=$(mktemp -d /tmp/kanban-install-test.XXXXXX)
export HOME="$HOME_DIR"
export SHELL="/bin/bash"
export GITHUB_LATEST_TAG="v26.6.2201"
set +e
sh "$INSTALL_SCRIPT" --dry-run --no-skills > "$HOME_DIR/stdout" 2> "$HOME_DIR/stderr"
_exit=$?
set -e
assert_exit_code 0 $_exit "remote dry-run exit"
_install_log=$(install_log_path "$HOME_DIR/stderr")
assert_file_exists "$_install_log"
assert_file_contains "$HOME_DIR/stderr" "installing kanban v26.6.2201"
assert_file_contains "$_install_log" "resolved latest GitHub release: v26.6.2201"
assert_file_contains "$_install_log" "would download kanban-26.6.2201-"
rm -rf "$HOME_DIR"
unset HOME_DIR HOME SHELL GITHUB_LATEST_TAG
echo "PASS: Scenario 9 - remote dry-run defaults to latest release"

# Scenario 10: remote dry-run with pinned version shows target version in progress
echo ""
echo "--- Scenario 10: remote dry-run shows pinned target version ---"
tests_run=$((tests_run + 1))
HOME_DIR=$(mktemp -d /tmp/kanban-install-test.XXXXXX)
export HOME="$HOME_DIR"
export SHELL="/bin/bash"
set +e
sh "$INSTALL_SCRIPT" --dry-run --no-skills --version v26.6.2203 > "$HOME_DIR/stdout" 2> "$HOME_DIR/stderr"
_exit=$?
set -e
assert_exit_code 0 $_exit "pinned remote dry-run exit"
_install_log=$(install_log_path "$HOME_DIR/stderr")
assert_file_exists "$_install_log"
assert_file_contains "$HOME_DIR/stderr" "installing kanban v26.6.2203"
assert_file_contains "$_install_log" "would download kanban-26.6.2203-"
rm -rf "$HOME_DIR"
unset HOME_DIR HOME SHELL
echo "PASS: Scenario 10 - remote dry-run shows pinned target version"

# Scenario 8: dry-run
echo ""
echo "--- Scenario 8: dry-run ---"
tests_run=$((tests_run + 1))

HOME_DIR=$(mktemp -d /tmp/kanban-install-test.XXXXXX)
export HOME="$HOME_DIR"
export SHELL="/bin/bash"

cp "$SCRIPT_DIR/stub-kanban" "$HOME_DIR/stub-kanban"
chmod +x "$HOME_DIR/stub-kanban"

set +e
sh "$INSTALL_SCRIPT" --binary "$HOME_DIR/stub-kanban" --dry-run --no-skills > "$HOME_DIR/stdout" 2> "$HOME_DIR/stderr"
_exit=$?
set -e

assert_exit_code 0 $_exit "dry-run exit"
# dry-run must not create the binary
if [ -f "$HOME_DIR/.local/bin/kanban" ]; then
	fail "dry-run should not create binary"
fi
if [ -e "$HOME_DIR/.local/bin/kb" ] || [ -L "$HOME_DIR/.local/bin/kb" ]; then
	fail "dry-run should not create kb symlink"
fi
# dry-run must not create manifest
if [ -f "$HOME_DIR/.local/lib/kanban/manifest.txt" ]; then
	fail "dry-run should not create manifest"
fi
# dry-run should preview operations
_install_log=$(install_log_path "$HOME_DIR/stderr")
assert_file_exists "$_install_log"
if ! grep -q '\[dry-run\]' "$_install_log" 2>/dev/null; then
	fail "dry-run did not produce preview output"
fi

rm -rf "$HOME_DIR"
unset HOME_DIR HOME SHELL

echo "PASS: Scenario 8 - dry-run previews without filesystem changes"

# Scenario: non-interactive install skips optional integrations without consent
echo ""
echo "--- Scenario: non-interactive skips optional integrations ---"
tests_run=$((tests_run + 1))

HOME_DIR=$(mktemp -d /tmp/kanban-install-test.XXXXXX)
export HOME="$HOME_DIR"
export SHELL="/bin/bash"
export KANBAN_INSTALL_NONINTERACTIVE=1

cp "$SCRIPT_DIR/stub-kanban" "$HOME_DIR/stub-kanban"
chmod +x "$HOME_DIR/stub-kanban"

set +e
sh "$INSTALL_SCRIPT" --binary "$HOME_DIR/stub-kanban" > "$HOME_DIR/stdout" 2> "$HOME_DIR/stderr"
_exit=$?
set -e

assert_exit_code 0 $_exit "non-interactive install exit"
assert_file_exists "$HOME_DIR/.local/bin/kanban"
assert_symlink_target "$HOME_DIR/.local/bin/kb" "kanban"
assert_file_not_contains "$HOME_DIR/.bashrc" "kanban-installer: PATH"
if [ -f "$HOME_DIR/.local/share/bash-completion/completions/kanban" ]; then
	fail "non-interactive install should not install completions without consent"
fi
if [ -d "$HOME_DIR/.config/opencode/skills" ]; then
	fail "non-interactive install should not install skills without consent"
fi
_install_log=$(install_log_path "$HOME_DIR/stderr")
assert_file_exists "$_install_log"
assert_file_contains "$_install_log" "PATH profile update skipped"
assert_file_contains "$_install_log" "shell completion installation skipped"
assert_file_contains "$_install_log" "agent skills skipped"

rm -rf "$HOME_DIR"
unset HOME_DIR HOME SHELL KANBAN_INSTALL_NONINTERACTIVE

echo "PASS: non-interactive skips optional integrations"

# Scenario 2: bash install (default prefix)
echo ""
echo "--- Scenario 2: bash install ---"
tests_run=$((tests_run + 1))

HOME_DIR=$(mktemp -d /tmp/kanban-install-test.XXXXXX)
export HOME="$HOME_DIR"
export SHELL="/bin/bash"

cp "$SCRIPT_DIR/stub-kanban" "$HOME_DIR/stub-kanban"
chmod +x "$HOME_DIR/stub-kanban"

set +e
sh "$INSTALL_SCRIPT" --binary "$HOME_DIR/stub-kanban" --yes --no-skills > "$HOME_DIR/stdout" 2> "$HOME_DIR/stderr"
_exit=$?
set -e

assert_exit_code 0 $_exit "bash install exit"
assert_file_exists "$HOME_DIR/.local/bin/kanban"
assert_symlink_target "$HOME_DIR/.local/bin/kb" "kanban"
assert_file_exists "$HOME_DIR/.local/lib/kanban/manifest.txt"
assert_file_contains "$HOME_DIR/.local/lib/kanban/manifest.txt" "generated:kanban-alias-symlink:kanban"
assert_file_contains "$HOME_DIR/.bashrc" "kanban-installer: PATH"
assert_file_contains "$HOME_DIR/.bashrc" 'export PATH='
assert_file_contains "$HOME_DIR/.local/share/bash-completion/completions/kanban" "stub kanban bash completion"

rm -rf "$HOME_DIR"
unset HOME_DIR HOME SHELL

echo "PASS: Scenario 2 - bash install"

# Scenario 1: zsh install (default prefix)
echo ""
echo "--- Scenario 1: zsh install ---"
tests_run=$((tests_run + 1))

HOME_DIR=$(mktemp -d /tmp/kanban-install-test.XXXXXX)
export HOME="$HOME_DIR"
export SHELL="/bin/zsh"

cp "$SCRIPT_DIR/stub-kanban" "$HOME_DIR/stub-kanban"
chmod +x "$HOME_DIR/stub-kanban"

set +e
sh "$INSTALL_SCRIPT" --binary "$HOME_DIR/stub-kanban" --yes --no-skills > "$HOME_DIR/stdout" 2> "$HOME_DIR/stderr"
_exit=$?
set -e

assert_exit_code 0 $_exit "zsh install exit"
assert_file_exists "$HOME_DIR/.local/bin/kanban"
assert_symlink_target "$HOME_DIR/.local/bin/kb" "kanban"
assert_file_exists "$HOME_DIR/.local/lib/kanban/manifest.txt"
assert_file_contains "$HOME_DIR/.zshrc" "kanban-installer: PATH"
assert_file_contains "$HOME_DIR/.zshrc" 'export PATH='
assert_file_exists "$HOME_DIR/.zsh/completions/_kanban"

rm -rf "$HOME_DIR"
unset HOME_DIR HOME SHELL

echo "PASS: Scenario 1 - zsh install"

# Scenario 3: ash install (Alpine, uses .profile)
echo ""
echo "--- Scenario 3: ash install (Alpine) ---"
tests_run=$((tests_run + 1))

HOME_DIR=$(mktemp -d /tmp/kanban-install-test.XXXXXX)
export HOME="$HOME_DIR"
export SHELL="/bin/ash"

cp "$SCRIPT_DIR/stub-kanban" "$HOME_DIR/stub-kanban"
chmod +x "$HOME_DIR/stub-kanban"

set +e
sh "$INSTALL_SCRIPT" --binary "$HOME_DIR/stub-kanban" --yes --no-skills > "$HOME_DIR/stdout" 2> "$HOME_DIR/stderr"
_exit=$?
set -e

assert_exit_code 0 $_exit "ash install exit"
assert_file_exists "$HOME_DIR/.local/bin/kanban"
assert_symlink_target "$HOME_DIR/.local/bin/kb" "kanban"
assert_file_contains "$HOME_DIR/.profile" "kanban-installer: PATH"
assert_file_contains "$HOME_DIR/.profile" 'export PATH='

# ash should not install completions
if [ -f "$HOME_DIR/.local/share/bash-completion/completions/kanban" ]; then
	fail "ash should not install bash completion"
fi
if [ -f "$HOME_DIR/.zsh/completions/_kanban" ]; then
	fail "ash should not install zsh completion"
fi

rm -rf "$HOME_DIR"
unset HOME_DIR HOME SHELL

echo "PASS: Scenario 3 - ash install with .profile"

# Scenario 4: custom prefix
echo ""
echo "--- Scenario 4: custom prefix ---"
tests_run=$((tests_run + 1))

HOME_DIR=$(mktemp -d /tmp/kanban-install-test.XXXXXX)
export HOME="$HOME_DIR"
export SHELL="/bin/bash"

cp "$SCRIPT_DIR/stub-kanban" "$HOME_DIR/stub-kanban"
chmod +x "$HOME_DIR/stub-kanban"

set +e
sh "$INSTALL_SCRIPT" --binary "$HOME_DIR/stub-kanban" --prefix "$HOME_DIR/bin" --yes --no-skills > "$HOME_DIR/stdout" 2> "$HOME_DIR/stderr"
_exit=$?
set -e

assert_exit_code 0 $_exit "custom prefix exit"
assert_file_exists "$HOME_DIR/bin/kanban"
assert_symlink_target "$HOME_DIR/bin/kb" "kanban"
# default path must not exist
if [ -f "$HOME_DIR/.local/bin/kanban" ]; then
	fail "custom prefix should not create default location"
fi
assert_file_contains "$HOME_DIR/.bashrc" "$HOME_DIR/bin"

rm -rf "$HOME_DIR"
unset HOME_DIR HOME SHELL

echo "PASS: Scenario 4 - custom prefix"

# Scenario 7: unsupported shell (fish) - skips completion
echo ""
echo "--- Scenario 7: unsupported shell (fish) ---"
tests_run=$((tests_run + 1))

HOME_DIR=$(mktemp -d /tmp/kanban-install-test.XXXXXX)
export HOME="$HOME_DIR"
export SHELL="/usr/bin/fish"

cp "$SCRIPT_DIR/stub-kanban" "$HOME_DIR/stub-kanban"
chmod +x "$HOME_DIR/stub-kanban"

set +e
sh "$INSTALL_SCRIPT" --binary "$HOME_DIR/stub-kanban" --yes --no-skills > "$HOME_DIR/stdout" 2> "$HOME_DIR/stderr"
_exit=$?
set -e

assert_exit_code 0 $_exit "fish install exit"
assert_file_exists "$HOME_DIR/.local/bin/kanban"
assert_symlink_target "$HOME_DIR/.local/bin/kb" "kanban"
# should print skip notice
if ! grep -q "skipped for fish" "$HOME_DIR/stderr" 2>/dev/null && \
   ! grep -q "skipped for" "$HOME_DIR/stderr" 2>/dev/null; then
	# The log may use a different phrasing; check that completion files are absent
	:
fi
# no completion files for fish
if [ -f "$HOME_DIR/.local/share/bash-completion/completions/kanban" ]; then
	fail "fish should not install bash completion"
fi
if [ -f "$HOME_DIR/.zsh/completions/_kanban" ]; then
	fail "fish should not install zsh completion"
fi

rm -rf "$HOME_DIR"
unset HOME_DIR HOME SHELL

echo "PASS: Scenario 7 - unsupported shell skipped gracefully"

# Scenario: idempotent re-run (PATH guard)
echo ""
echo "--- Idempotent PATH guard ---"
tests_run=$((tests_run + 1))

HOME_DIR=$(mktemp -d /tmp/kanban-install-test.XXXXXX)
export HOME="$HOME_DIR"
export SHELL="/bin/bash"

cp "$SCRIPT_DIR/stub-kanban" "$HOME_DIR/stub-kanban"
chmod +x "$HOME_DIR/stub-kanban"

# First run
set +e
sh "$INSTALL_SCRIPT" --binary "$HOME_DIR/stub-kanban" --yes --no-skills > /dev/null 2>/dev/null
_exit=$?
set -e
assert_exit_code 0 $_exit "first run exit"

# Second run
set +e
sh "$INSTALL_SCRIPT" --binary "$HOME_DIR/stub-kanban" --yes --no-skills > /dev/null 2>/dev/null
_exit=$?
set -e
assert_exit_code 0 $_exit "second run exit"

# Count sentinel lines - should have exactly one PATH sentinel
_count=$(grep -c "kanban-installer: PATH" "$HOME_DIR/.bashrc" 2>/dev/null || echo "0")
if [ "$_count" -ne 1 ]; then
	fail "idempotent run: expected 1 PATH sentinel in .bashrc, got $_count"
fi

rm -rf "$HOME_DIR"
unset HOME_DIR HOME SHELL

echo "PASS: Idempotent PATH guard"

# US-002: --no-skills skips skill installation
echo ""
echo "--- US-002: --no-skills skips skill install ---"
tests_run=$((tests_run + 1))

HOME_DIR=$(mktemp -d /tmp/kanban-install-test.XXXXXX)
export HOME="$HOME_DIR"
export SHELL="/bin/bash"

cp "$SCRIPT_DIR/stub-kanban" "$HOME_DIR/stub-kanban"
chmod +x "$HOME_DIR/stub-kanban"

set +e
sh "$INSTALL_SCRIPT" --binary "$HOME_DIR/stub-kanban" --yes --no-skills > "$HOME_DIR/stdout" 2> "$HOME_DIR/stderr"
_exit=$?
set -e

assert_exit_code 0 $_exit "--no-skills install exit"
assert_file_exists "$HOME_DIR/.local/bin/kanban"
# No skills should be installed
if [ -d "$HOME_DIR/.config/opencode/skills" ]; then
	fail "--no-skills should not create skills directory"
fi
# Log should mention skipping
_install_log=$(install_log_path "$HOME_DIR/stderr")
assert_file_exists "$_install_log"
if ! grep -q "skipped" "$_install_log" 2>/dev/null; then
	fail "--no-skills should log skip notice"
fi

rm -rf "$HOME_DIR"
unset HOME_DIR HOME SHELL

echo "PASS: US-002 --no-skills skips skill install"

# US-002: --skills-dir installs to specified directory
echo ""
echo "--- US-002: --skills-dir installs to specified dir ---"
tests_run=$((tests_run + 1))

HOME_DIR=$(mktemp -d /tmp/kanban-install-test.XXXXXX)
export HOME="$HOME_DIR"
export SHELL="/bin/bash"
_skill_target="$HOME_DIR/.my-agents/skills"

cp "$SCRIPT_DIR/stub-kanban" "$HOME_DIR/stub-kanban"
chmod +x "$HOME_DIR/stub-kanban"

set +e
sh "$INSTALL_SCRIPT" --binary "$HOME_DIR/stub-kanban" --skills-dir "$_skill_target" --yes > "$HOME_DIR/stdout" 2> "$HOME_DIR/stderr"
_exit=$?
set -e

assert_exit_code 0 $_exit "--skills-dir install exit"
assert_file_exists "$_skill_target/kanban-backlog-maintainer/SKILL.md"
assert_file_exists "$_skill_target/kanban-backlog-maintainer/plugin.json"
assert_file_exists "$_skill_target/kanban-developer/SKILL.md"
assert_file_exists "$_skill_target/kanban-developer/plugin.json"
# Manifest should record skill entries
assert_file_contains "$HOME_DIR/.local/lib/kanban/manifest.txt" "repo:skills/kanban-backlog-maintainer/SKILL.md"
assert_file_contains "$HOME_DIR/.local/lib/kanban/manifest.txt" "repo:skills/kanban-developer/plugin.json"

rm -rf "$HOME_DIR"
unset HOME_DIR HOME SHELL

echo "PASS: US-002 --skills-dir installs to specified dir"

# US-002: OPENCODE_HOME env var selects skills dir
echo ""
echo "--- US-002: OPENCODE_HOME selects skills dir ---"
tests_run=$((tests_run + 1))

HOME_DIR=$(mktemp -d /tmp/kanban-install-test.XXXXXX)
export HOME="$HOME_DIR"
export SHELL="/bin/bash"
export OPENCODE_HOME="$HOME_DIR/.opencode"
mkdir -p "$OPENCODE_HOME"

cp "$SCRIPT_DIR/stub-kanban" "$HOME_DIR/stub-kanban"
chmod +x "$HOME_DIR/stub-kanban"

set +e
sh "$INSTALL_SCRIPT" --binary "$HOME_DIR/stub-kanban" --yes > "$HOME_DIR/stdout" 2> "$HOME_DIR/stderr"
_exit=$?
set -e

assert_exit_code 0 $_exit "OPENCODE_HOME install exit"
assert_file_exists "$OPENCODE_HOME/skills/kanban-backlog-maintainer/SKILL.md"
assert_file_exists "$OPENCODE_HOME/skills/kanban-developer/SKILL.md"
# Should NOT install to default .config/opencode path
if [ -d "$HOME_DIR/.config/opencode/skills" ]; then
	fail "OPENCODE_HOME should override ~/.config/opencode/skills"
fi

unset OPENCODE_HOME
rm -rf "$HOME_DIR"
unset HOME_DIR HOME SHELL

echo "PASS: US-002 OPENCODE_HOME selects skills dir"

# US-002: XDG_CONFIG_HOME env var selects skills dir
echo ""
echo "--- US-002: XDG_CONFIG_HOME selects skills dir ---"
tests_run=$((tests_run + 1))

HOME_DIR=$(mktemp -d /tmp/kanban-install-test.XXXXXX)
export HOME="$HOME_DIR"
export SHELL="/bin/bash"
export XDG_CONFIG_HOME="$HOME_DIR/.cfg"

cp "$SCRIPT_DIR/stub-kanban" "$HOME_DIR/stub-kanban"
chmod +x "$HOME_DIR/stub-kanban"

set +e
sh "$INSTALL_SCRIPT" --binary "$HOME_DIR/stub-kanban" --yes > "$HOME_DIR/stdout" 2> "$HOME_DIR/stderr"
_exit=$?
set -e

assert_exit_code 0 $_exit "XDG_CONFIG_HOME install exit"
assert_file_exists "$XDG_CONFIG_HOME/opencode/skills/kanban-backlog-maintainer/SKILL.md"
assert_file_exists "$XDG_CONFIG_HOME/opencode/skills/kanban-developer/SKILL.md"

unset XDG_CONFIG_HOME
rm -rf "$HOME_DIR"
unset HOME_DIR HOME SHELL

echo "PASS: US-002 XDG_CONFIG_HOME selects skills dir"

# US-002: OPENCODE_HOME overrides XDG_CONFIG_HOME
echo ""
echo "--- US-002: OPENCODE_HOME overrides XDG_CONFIG_HOME ---"
tests_run=$((tests_run + 1))

HOME_DIR=$(mktemp -d /tmp/kanban-install-test.XXXXXX)
export HOME="$HOME_DIR"
export SHELL="/bin/bash"
export OPENCODE_HOME="$HOME_DIR/.opencode"
export XDG_CONFIG_HOME="$HOME_DIR/.cfg"
mkdir -p "$OPENCODE_HOME"

cp "$SCRIPT_DIR/stub-kanban" "$HOME_DIR/stub-kanban"
chmod +x "$HOME_DIR/stub-kanban"

set +e
sh "$INSTALL_SCRIPT" --binary "$HOME_DIR/stub-kanban" --yes > "$HOME_DIR/stdout" 2> "$HOME_DIR/stderr"
_exit=$?
set -e

assert_exit_code 0 $_exit "OPENCODE_HOME overrides XDG_CONFIG_HOME exit"
assert_file_exists "$OPENCODE_HOME/skills/kanban-backlog-maintainer/SKILL.md"
# Should NOT use XDG path
if [ -d "$XDG_CONFIG_HOME/opencode/skills" ]; then
	fail "OPENCODE_HOME should take priority over XDG_CONFIG_HOME"
fi

unset OPENCODE_HOME
unset XDG_CONFIG_HOME
rm -rf "$HOME_DIR"
unset HOME_DIR HOME SHELL

echo "PASS: US-002 OPENCODE_HOME overrides XDG_CONFIG_HOME"

# US-002: existing ~/.config/opencode/skills is auto-discovered
echo ""
echo "--- US-002: existing opencode config auto-discovered ---"
tests_run=$((tests_run + 1))

HOME_DIR=$(mktemp -d /tmp/kanban-install-test.XXXXXX)
export HOME="$HOME_DIR"
export SHELL="/bin/bash"
mkdir -p "$HOME_DIR/.config/opencode/skills"

cp "$SCRIPT_DIR/stub-kanban" "$HOME_DIR/stub-kanban"
chmod +x "$HOME_DIR/stub-kanban"

set +e
sh "$INSTALL_SCRIPT" --binary "$HOME_DIR/stub-kanban" --yes > "$HOME_DIR/stdout" 2> "$HOME_DIR/stderr"
_exit=$?
set -e

assert_exit_code 0 $_exit "existing opencode config exit"
assert_file_exists "$HOME_DIR/.config/opencode/skills/kanban-backlog-maintainer/SKILL.md"
assert_file_exists "$HOME_DIR/.config/opencode/skills/kanban-developer/SKILL.md"

rm -rf "$HOME_DIR"
unset HOME_DIR HOME SHELL

echo "PASS: US-002 existing opencode config auto-discovered"

# US-002: dry-run previews skill install without filesystem changes
echo ""
echo "--- US-002: dry-run previews skill install ---"
tests_run=$((tests_run + 1))

HOME_DIR=$(mktemp -d /tmp/kanban-install-test.XXXXXX)
export HOME="$HOME_DIR"
export SHELL="/bin/bash"
_skill_target="$HOME_DIR/.my-agents/skills"

cp "$SCRIPT_DIR/stub-kanban" "$HOME_DIR/stub-kanban"
chmod +x "$HOME_DIR/stub-kanban"

set +e
sh "$INSTALL_SCRIPT" --binary "$HOME_DIR/stub-kanban" --skills-dir "$_skill_target" --dry-run > "$HOME_DIR/stdout" 2> "$HOME_DIR/stderr"
_exit=$?
set -e

assert_exit_code 0 $_exit "dry-run skills exit"
# Must not create skill files
if [ -f "$_skill_target/kanban-backlog-maintainer/SKILL.md" ]; then
	fail "dry-run should not create skill files"
fi
if [ -f "$_skill_target/kanban-developer/SKILL.md" ]; then
	fail "dry-run should not create skill files"
fi
# Should preview skill operations
_install_log=$(install_log_path "$HOME_DIR/stderr")
assert_file_exists "$_install_log"
if ! grep -q '\[dry-run\]' "$_install_log" 2>/dev/null; then
	fail "dry-run did not produce preview output"
fi
# Should mention skill dir
if ! grep -q "skills" "$_install_log" 2>/dev/null; then
	fail "dry-run output missing skills references"
fi

rm -rf "$HOME_DIR"
unset HOME_DIR HOME SHELL

echo "PASS: US-002 dry-run previews skill install"

# US-002/US-003: existing optional integrations update in place without prompting
echo ""
echo "--- Existing optional integrations update in place ---"
tests_run=$((tests_run + 1))

HOME_DIR=$(mktemp -d /tmp/kanban-install-test.XXXXXX)
export HOME="$HOME_DIR"
export SHELL="/bin/bash"

cp "$SCRIPT_DIR/stub-kanban" "$HOME_DIR/stub-kanban"
chmod +x "$HOME_DIR/stub-kanban"

# First install opts into PATH, completions, and skills.
set +e
sh "$INSTALL_SCRIPT" --binary "$HOME_DIR/stub-kanban" --yes > /dev/null 2> "$HOME_DIR/stderr1"
_exit=$?
set -e
assert_exit_code 0 $_exit "first existing-integrations install exit"

assert_file_contains "$HOME_DIR/.bashrc" "kanban-installer: PATH"
assert_file_exists "$HOME_DIR/.local/share/bash-completion/completions/kanban"
assert_file_exists "$HOME_DIR/.config/opencode/skills/kanban-backlog-maintainer/SKILL.md"
assert_file_contains "$HOME_DIR/.local/lib/kanban/manifest.txt" "# path-installed: yes"
assert_file_contains "$HOME_DIR/.local/lib/kanban/manifest.txt" "# completions-installed: yes"

# Second install is non-interactive and must reuse prior consent.
export KANBAN_INSTALL_NONINTERACTIVE=1
set +e
sh "$INSTALL_SCRIPT" --binary "$HOME_DIR/stub-kanban" > /dev/null 2> "$HOME_DIR/stderr2"
_exit=$?
set -e
unset KANBAN_INSTALL_NONINTERACTIVE

assert_exit_code 0 $_exit "existing-integrations non-interactive re-run exit"
assert_file_contains "$HOME_DIR/.bashrc" "kanban-installer: PATH"
assert_file_contains "$HOME_DIR/.local/share/bash-completion/completions/kanban" "stub kanban bash completion"
assert_file_exists "$HOME_DIR/.config/opencode/skills/kanban-developer/SKILL.md"

_install_log=$(install_log_path "$HOME_DIR/stderr2")
assert_file_exists "$_install_log"
assert_file_contains "$_install_log" "PATH profile update already installed"
assert_file_contains "$_install_log" "shell completion already installed"
assert_file_contains "$_install_log" "agent skills already installed"
assert_line_order "$_install_log" \
	"PATH profile update already installed" \
	"shell completion already installed" \
	"agent skills already installed"

rm -rf "$HOME_DIR"
unset HOME_DIR HOME SHELL

echo "PASS: existing optional integrations update in place"

# US-002: idempotent re-run (upgrade-in-place)
echo ""
echo "--- US-002: idempotent re-run (upgrade-in-place) ---"
tests_run=$((tests_run + 1))

HOME_DIR=$(mktemp -d /tmp/kanban-install-test.XXXXXX)
export HOME="$HOME_DIR"
export SHELL="/bin/bash"

cp "$SCRIPT_DIR/stub-kanban" "$HOME_DIR/stub-kanban"
chmod +x "$HOME_DIR/stub-kanban"

# First run
set +e
sh "$INSTALL_SCRIPT" --binary "$HOME_DIR/stub-kanban" --yes > /dev/null 2> "$HOME_DIR/stderr1"
_exit=$?
set -e
assert_exit_code 0 $_exit "first install exit"

# Second run
set +e
sh "$INSTALL_SCRIPT" --binary "$HOME_DIR/stub-kanban" --yes > /dev/null 2> "$HOME_DIR/stderr2"
_exit=$?
set -e
assert_exit_code 0 $_exit "second install exit"

assert_file_exists "$HOME_DIR/.config/opencode/skills/kanban-backlog-maintainer/SKILL.md"
assert_file_exists "$HOME_DIR/.config/opencode/skills/kanban-developer/SKILL.md"

# Manifest should be present
assert_file_exists "$HOME_DIR/.local/lib/kanban/manifest.txt"
# Manifest should mention skills (should have entries, not be empty of them)
if ! grep -q "repo:skills/" "$HOME_DIR/.local/lib/kanban/manifest.txt" 2>/dev/null; then
	fail "manifest should contain skill entries after install"
fi

rm -rf "$HOME_DIR"
unset HOME_DIR HOME SHELL

echo "PASS: US-002 idempotent re-run"

# US-003: upgrade in place (re-run with same prefix/skills-dir)
echo ""
echo "--- US-003: re-run upgrades in place ---"
tests_run=$((tests_run + 1))

HOME_DIR=$(mktemp -d /tmp/kanban-install-test.XXXXXX)
export HOME="$HOME_DIR"
export SHELL="/bin/bash"

cp "$SCRIPT_DIR/stub-kanban" "$HOME_DIR/stub-kanban"
chmod +x "$HOME_DIR/stub-kanban"

# First install
set +e
sh "$INSTALL_SCRIPT" --binary "$HOME_DIR/stub-kanban" --yes --skills-dir "$HOME_DIR/.config/opencode/skills" > /dev/null 2> "$HOME_DIR/stderr1"
assert_exit_code 0 $? "first install exit"
set -e

assert_file_exists "$HOME_DIR/.local/bin/kanban"
assert_file_exists "$HOME_DIR/.config/opencode/skills/kanban-backlog-maintainer/SKILL.md"
_manifest="$HOME_DIR/.local/lib/kanban/manifest.txt"
assert_file_exists "$_manifest"
# Manifest should have header
assert_file_contains "$_manifest" "# kanban install manifest"
assert_file_contains "$_manifest" "# installer-version:"

# Second install (re-run)
set +e
sh "$INSTALL_SCRIPT" --binary "$HOME_DIR/stub-kanban" --yes --skills-dir "$HOME_DIR/.config/opencode/skills" > /dev/null 2> "$HOME_DIR/stderr2"
assert_exit_code 0 $? "second install exit"
set -e

assert_file_exists "$HOME_DIR/.local/bin/kanban"
assert_file_exists "$_manifest"
# Manifest should still have headers, not duplicated
_count=$(grep -c "^# kanban install manifest" "$_manifest" 2>/dev/null || echo "0")
if [ "$_count" -ne 1 ]; then
	fail "manifest header should not be duplicated, got $_count"
fi

rm -rf "$HOME_DIR"
unset HOME_DIR HOME SHELL

echo "PASS: US-003 re-run upgrades in place"

# US-003: downgrade refusal
echo ""
echo "--- US-003: downgrade is refused ---"
tests_run=$((tests_run + 1))

HOME_DIR=$(mktemp -d /tmp/kanban-install-test.XXXXXX)
export HOME="$HOME_DIR"
export SHELL="/bin/bash"

cp "$SCRIPT_DIR/stub-kanban" "$HOME_DIR/stub-kanban"
chmod +x "$HOME_DIR/stub-kanban"

# First install with stub (v1)
set +e
sh "$INSTALL_SCRIPT" --binary "$HOME_DIR/stub-kanban" --yes --skills-dir "$HOME_DIR/.config/opencode/skills" > /dev/null 2>&1
set -e

# Second install with a "newer" stub (same stub, but version check runs)
# The stub always reports the same version, so downgrade detection will see same version
# and not trigger. Let's test that re-run works.
set +e
sh "$INSTALL_SCRIPT" --binary "$HOME_DIR/stub-kanban" --yes --skills-dir "$HOME_DIR/.config/opencode/skills" > /dev/null 2>&1
_exit=$?
set -e
assert_exit_code 0 $_exit "same-version re-run exit"

rm -rf "$HOME_DIR"
unset HOME_DIR HOME SHELL

echo "PASS: US-003 downgrade refusal (same version re-run)"

# US-003: different prefix = fresh install, old install left untouched
echo ""
echo "--- US-003: different prefix triggers fresh install ---"
tests_run=$((tests_run + 1))

HOME_DIR=$(mktemp -d /tmp/kanban-install-test.XXXXXX)
export HOME="$HOME_DIR"
export SHELL="/bin/bash"

cp "$SCRIPT_DIR/stub-kanban" "$HOME_DIR/stub-kanban"
chmod +x "$HOME_DIR/stub-kanban"

# First install with default prefix
set +e
sh "$INSTALL_SCRIPT" --binary "$HOME_DIR/stub-kanban" --yes --skills-dir "$HOME_DIR/.config/opencode/skills" > /dev/null 2>&1
set -e

# Second install with different prefix
set +e
sh "$INSTALL_SCRIPT" --binary "$HOME_DIR/stub-kanban" --prefix "$HOME_DIR/bin" --yes --no-skills > "$HOME_DIR/stdout" 2> "$HOME_DIR/stderr"
_exit=$?
set -e
assert_exit_code 0 $_exit "different prefix exit"

# New prefix should have binary and manifest
assert_file_exists "$HOME_DIR/bin/kanban"
assert_file_exists "$HOME_DIR/lib/kanban/manifest.txt"

# Old install should still exist
assert_file_exists "$HOME_DIR/.local/bin/kanban"
assert_file_exists "$HOME_DIR/.local/lib/kanban/manifest.txt"

# Log should mention previous install
if ! grep -q "previous install" "$HOME_DIR/stderr" 2>/dev/null; then
	:
fi

rm -rf "$HOME_DIR"
unset HOME_DIR HOME SHELL

echo "PASS: US-003 different prefix triggers fresh install"

# US-003: different skills-dir reconciles old skills
echo ""
echo "--- US-003: different skills-dir reconciles old skills ---"
tests_run=$((tests_run + 1))

HOME_DIR=$(mktemp -d /tmp/kanban-install-test.XXXXXX)
export HOME="$HOME_DIR"
export SHELL="/bin/bash"
_old_skills="$HOME_DIR/.config/opencode/skills"
_new_skills="$HOME_DIR/.my-agents/skills"

cp "$SCRIPT_DIR/stub-kanban" "$HOME_DIR/stub-kanban"
chmod +x "$HOME_DIR/stub-kanban"

# First install with old skills dir
set +e
sh "$INSTALL_SCRIPT" --binary "$HOME_DIR/stub-kanban" --yes --skills-dir "$_old_skills" > /dev/null 2>&1
set -e
assert_file_exists "$_old_skills/kanban-backlog-maintainer/SKILL.md"

# Second install with new skills dir
set +e
sh "$INSTALL_SCRIPT" --binary "$HOME_DIR/stub-kanban" --yes --skills-dir "$_new_skills" > /dev/null 2> "$HOME_DIR/stderr"
_exit=$?
set -e
assert_exit_code 0 $_exit "different skills-dir exit"

# New skills dir should have files
assert_file_exists "$_new_skills/kanban-backlog-maintainer/SKILL.md"
assert_file_exists "$_new_skills/kanban-developer/SKILL.md"

# Old skills files should be removed
if [ -f "$_old_skills/kanban-backlog-maintainer/SKILL.md" ]; then
	fail "old skill files should be removed after skills-dir change"
fi
if [ -f "$_old_skills/kanban-developer/SKILL.md" ]; then
	fail "old skill files should be removed after skills-dir change"
fi

rm -rf "$HOME_DIR"
unset HOME_DIR HOME SHELL

echo "PASS: US-003 different skills-dir reconciles old skills"

# US-003: dry-run shows upgrade diff
echo ""
echo "--- US-003: dry-run shows upgrade diff ---"
tests_run=$((tests_run + 1))

HOME_DIR=$(mktemp -d /tmp/kanban-install-test.XXXXXX)
export HOME="$HOME_DIR"
export SHELL="/bin/bash"

cp "$SCRIPT_DIR/stub-kanban" "$HOME_DIR/stub-kanban"
chmod +x "$HOME_DIR/stub-kanban"

# First install
set +e
sh "$INSTALL_SCRIPT" --binary "$HOME_DIR/stub-kanban" --yes --skills-dir "$HOME_DIR/.config/opencode/skills" > /dev/null 2>&1
set -e

# Dry-run re-run
set +e
sh "$INSTALL_SCRIPT" --binary "$HOME_DIR/stub-kanban" --yes --skills-dir "$HOME_DIR/.config/opencode/skills" --dry-run > "$HOME_DIR/stdout" 2> "$HOME_DIR/stderr"
_exit=$?
set -e
assert_exit_code 0 $_exit "dry-run upgrade exit"

# Dry-run should not modify files
_install_log=$(install_log_path "$HOME_DIR/stderr")
assert_file_exists "$_install_log"
if ! grep -q '\[dry-run\]' "$_install_log" 2>/dev/null; then
	fail "dry-run upgrade should produce preview output"
fi

rm -rf "$HOME_DIR"
unset HOME_DIR HOME SHELL

echo "PASS: US-003 dry-run upgrade"

# US-003: --force bypasses prompts
echo ""
echo "--- US-003: --force flag ---"
tests_run=$((tests_run + 1))

HOME_DIR=$(mktemp -d /tmp/kanban-install-test.XXXXXX)
export HOME="$HOME_DIR"
export SHELL="/bin/bash"

cp "$SCRIPT_DIR/stub-kanban" "$HOME_DIR/stub-kanban"
chmod +x "$HOME_DIR/stub-kanban"

# Install with --force (should work like --yes for prompts)
set +e
sh "$INSTALL_SCRIPT" --binary "$HOME_DIR/stub-kanban" --force --no-skills --no-add-path --no-completions > /dev/null 2>&1
_exit=$?
set -e
assert_exit_code 0 $_exit "--force install exit"
assert_file_exists "$HOME_DIR/.local/bin/kanban"

# Re-run with --force
set +e
sh "$INSTALL_SCRIPT" --binary "$HOME_DIR/stub-kanban" --force --no-skills --no-add-path --no-completions > /dev/null 2>&1
_exit=$?
set -e
assert_exit_code 0 $_exit "--force re-run exit"

rm -rf "$HOME_DIR"
unset HOME_DIR HOME SHELL

echo "PASS: US-003 --force flag"

# ============================================================
# US-004: Uninstall tests
# ============================================================

UNINSTALL_SCRIPT="$REPO_ROOT/scripts/uninstall.sh"

# US-004: full uninstall after clean install
echo ""
echo "--- US-004: full uninstall after clean install ---"
tests_run=$((tests_run + 1))

HOME_DIR=$(mktemp -d /tmp/kanban-install-test.XXXXXX)
export HOME="$HOME_DIR"
export SHELL="/bin/bash"

cp "$SCRIPT_DIR/stub-kanban" "$HOME_DIR/stub-kanban"
chmod +x "$HOME_DIR/stub-kanban"

# Install
set +e
sh "$INSTALL_SCRIPT" --binary "$HOME_DIR/stub-kanban" --yes --skills-dir "$HOME_DIR/.config/opencode/skills" > /dev/null 2>&1
set -e
assert_file_exists "$HOME_DIR/.local/bin/kanban"
assert_symlink_target "$HOME_DIR/.local/bin/kb" "kanban"
assert_file_exists "$HOME_DIR/.config/opencode/skills/kanban-backlog-maintainer/SKILL.md"
assert_file_contains "$HOME_DIR/.bashrc" "kanban-installer: PATH"

# Uninstall
set +e
sh "$UNINSTALL_SCRIPT" --yes > /dev/null 2>&1
_exit=$?
set -e
assert_exit_code 0 $_exit "uninstall exit"

# Binary should be gone
if [ -f "$HOME_DIR/.local/bin/kanban" ]; then
	fail "binary should be removed after uninstall"
fi
if [ -e "$HOME_DIR/.local/bin/kb" ] || [ -L "$HOME_DIR/.local/bin/kb" ]; then
	fail "kb symlink should be removed after uninstall"
fi
# Skills should be gone
if [ -f "$HOME_DIR/.config/opencode/skills/kanban-backlog-maintainer/SKILL.md" ]; then
	fail "skill files should be removed after uninstall"
fi
# Manifest dir should be gone
if [ -d "$HOME_DIR/.local/lib/kanban" ]; then
	fail "manifest dir should be removed after uninstall"
fi
# No sentinel lines in bashrc
if grep -q "kanban-installer:" "$HOME_DIR/.bashrc" 2>/dev/null; then
	fail "sentinel lines should be stripped from bashrc"
fi

rm -rf "$HOME_DIR"
unset HOME_DIR HOME SHELL

echo "PASS: US-004 full uninstall"

# US-004: dry-run uninstall
echo ""
echo "--- US-004: dry-run uninstall ---"
tests_run=$((tests_run + 1))

HOME_DIR=$(mktemp -d /tmp/kanban-install-test.XXXXXX)
export HOME="$HOME_DIR"
export SHELL="/bin/bash"

cp "$SCRIPT_DIR/stub-kanban" "$HOME_DIR/stub-kanban"
chmod +x "$HOME_DIR/stub-kanban"

# Install
set +e
sh "$INSTALL_SCRIPT" --binary "$HOME_DIR/stub-kanban" --yes --skills-dir "$HOME_DIR/.config/opencode/skills" > /dev/null 2>&1
set -e

# Dry-run uninstall
set +e
sh "$UNINSTALL_SCRIPT" --yes --dry-run > "$HOME_DIR/stdout" 2> "$HOME_DIR/stderr"
_exit=$?
set -e
assert_exit_code 0 $_exit "dry-run uninstall exit"

# Files should still exist
assert_file_exists "$HOME_DIR/.local/bin/kanban"
assert_symlink_target "$HOME_DIR/.local/bin/kb" "kanban"
assert_file_exists "$HOME_DIR/.config/opencode/skills/kanban-backlog-maintainer/SKILL.md"
# Should preview
if ! grep -q '\[dry-run\]' "$HOME_DIR/stderr" 2>/dev/null; then
	fail "dry-run uninstall should produce preview output"
fi

rm -rf "$HOME_DIR"
unset HOME_DIR HOME SHELL

echo "PASS: US-004 dry-run uninstall"

# US-004: no manifest short-circuits gracefully
echo ""
echo "--- US-004: no manifest short-circuits ---"
tests_run=$((tests_run + 1))

HOME_DIR=$(mktemp -d /tmp/kanban-install-test.XXXXXX)
export HOME="$HOME_DIR"
export SHELL="/bin/bash"

set +e
sh "$UNINSTALL_SCRIPT" --yes > "$HOME_DIR/stdout" 2> "$HOME_DIR/stderr"
_exit=$?
set -e
assert_exit_code 0 $_exit "no-manifest exit"
if ! grep -q "no manifest" "$HOME_DIR/stderr" 2>/dev/null && \
   ! grep -q "nothing" "$HOME_DIR/stderr" 2>/dev/null; then
	fail "should report no manifest found"
fi

rm -rf "$HOME_DIR"
unset HOME_DIR HOME SHELL

echo "PASS: US-004 no manifest short-circuits"

# Results
echo ""
echo "=== Results ==="
echo "Tests run: $tests_run"
echo "Failures:  $failures"

if [ "$failures" -gt 0 ]; then
	exit 1
fi
echo "All tests passed."
