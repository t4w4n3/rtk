#!/usr/bin/env bash
# .claude/hooks/post-edit-analysis.sh
#
# PostToolUse hook: auto-fix + static analysis on modified files.
# - systemMessage to stdout  → visible notification in Claude Code UI
# - exit 2 + stderr          → blocks action, sends errors back to Claude
#
# Supported extensions:
#   .rs  → cargo check (block on error) + cargo clippy (warn on warnings)
#   .sh  → shfmt auto-fix + shellcheck (block on error)

set -euo pipefail

INPUT=$(cat)
FILE=$(echo "$INPUT" | jq -r '.tool_input.file_path // empty')

[[ -z "$FILE" ]] && exit 0

cd "$CLAUDE_PROJECT_DIR"

FIXED=""
GATE=""

# Run a tool and record its name if it modified the file
run_fix() {
	local tool="$1"
	shift
	local before after
	before=$(shasum "$FILE")
	"$@" >/dev/null 2>&1 || true
	after=$(shasum "$FILE")
	if [[ "$before" != "$after" ]]; then
		FIXED="${FIXED:+$FIXED, }$tool"
	fi
}

# Block the action and send error details back to Claude
fail() {
	local label="$1" errors="$2"
	# Embed errors inside systemMessage so Claude Code surfaces them.
	# jq builds valid JSON and escapes newlines/quotes in the error output.
	local prefix="${FIXED:+auto-fixed by $FIXED | }"
	jq -n --arg msg "${prefix}${label}" --arg err "$errors" \
		'{"systemMessage": ($msg + "\n" + $err)}' >&2
	exit 2
}

gate_shellcheck() {
	local errors
	if ! shellcheck --version &>/dev/null; then
		GATE="${GATE:+$GATE | }shellcheck: not installed (brew install shellcheck)"
		return
	fi
	if errors=$(shellcheck "$FILE" 2>&1); then
		GATE="${GATE:+$GATE | }shellcheck: OK"
	else
		fail "shellcheck: FAILED on ${FILE##*/}" "$errors"
	fi
}

gate_cargo_check() {
	local raw
	raw=$(cargo check 2>&1)
	local exit_code=$?
	local errors
	errors=$(echo "$raw" | grep -v -E "^\s+(Compiling|Checking|Finished|Downloaded|Downloading)")
	if [[ $exit_code -ne 0 ]]; then
		fail "cargo check: FAILED on ${FILE##*/}" "$errors"
	else
		GATE="${GATE:+$GATE | }cargo check: OK"
	fi
}

gate_cargo_clippy() {
	local raw
	raw=$(cargo clippy 2>&1)
	local clippy_exit=$?
	local output
	output=$(echo "$raw" | grep -v -E "^\s+(Compiling|Checking|Finished|Downloaded|Downloading)")
	if [[ $clippy_exit -ne 0 ]] || echo "$output" | grep -qE "^error"; then
		fail "cargo clippy: FAILED on ${FILE##*/}" "$output"
	elif echo "$output" | grep -qE "^warning"; then
		GATE="${GATE:+$GATE | }cargo clippy: warnings"
	else
		GATE="${GATE:+$GATE | }cargo clippy: OK"
	fi
}

case "$FILE" in
*.rs)
	gate_cargo_check
	gate_cargo_clippy
	;;
*.sh)
	run_fix "shfmt" shfmt -w "$FILE"
	gate_shellcheck
	;;
esac

# Emit user-visible notification
if [[ -n "$FIXED" || -n "$GATE" ]]; then
	echo "{\"systemMessage\": \"${FIXED:+auto-fixed by $FIXED | }${GATE}\"}"
fi
