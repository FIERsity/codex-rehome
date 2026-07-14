#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
BIN=${CODEX_REHOME_BIN:-"$ROOT/target/release/codex-rehome"}
SCHEMA="$ROOT/tests/fixtures/codex-0.144.2/schema.sql"
TMP=$(mktemp -d "${TMPDIR:-/tmp}/codex-rehome-e2e.XXXXXX")
trap 'rm -rf "$TMP"' EXIT HUP INT TERM
TMP=$(CDPATH= cd -- "$TMP" && pwd -P)

HOME_DIR="$TMP/codex-home"
OLD="$TMP/old project"
NEW="$TMP/new project"
ROLLOUT="$HOME_DIR/sessions/2026/07/14/rollout.jsonl"
FAKE_BIN="$TMP/bin"

fail() {
  echo "disposable e2e: $*" >&2
  exit 1
}

[ -x "$BIN" ] || fail "release binary not found: $BIN"
command -v sqlite3 >/dev/null 2>&1 || fail "sqlite3 is required"

mkdir -p "$HOME_DIR/sessions/2026/07/14" "$OLD" "$NEW" "$FAKE_BIN"
printf '%s\n' '#!/bin/sh' 'exit 1' >"$FAKE_BIN/pgrep"
chmod 755 "$FAKE_BIN/pgrep"
printf '%s\n' '#!/bin/sh' 'echo codex-cli 0.144.2' >"$FAKE_BIN/codex"
chmod 755 "$FAKE_BIN/codex"
printf '%s\n' 'disposable project marker' >"$OLD/marker.txt"

sqlite3 "$HOME_DIR/state_5.sqlite" <"$SCHEMA"
sqlite3 "$HOME_DIR/state_5.sqlite" \
  "INSERT INTO threads(id,rollout_path,created_at,updated_at,source,model_provider,cwd,title,sandbox_policy,approval_mode) VALUES('e2e-thread','$ROLLOUT',0,0,'cli','openai','$OLD','synthetic','{}','never');"
printf '%s\n' \
  "{\"type\":\"session_meta\",\"payload\":{\"id\":\"e2e-thread\",\"cwd\":\"$OLD\"}}" \
  '{"type":"response_item","payload":{"content":"unrelated prose stays unchanged"}}' >"$ROLLOUT"
printf '%s\n' \
  "{\"active-workspace-roots\":[\"$OLD\"],\"thread-workspace-root-hints\":{\"e2e-thread\":\"$OLD/sub\"}}" \
  >"$HOME_DIR/.codex-global-state.json"

run() {
  CODEX_HOME="$HOME_DIR" PATH="$FAKE_BIN:/usr/bin:/bin" "$BIN" "$@"
}

run doctor >/dev/null
run inspect "$OLD" >/dev/null
run plan "$OLD" "$NEW" >/dev/null

REMAP_OUTPUT=$(run remap "$OLD" "$NEW" --yes)
REMAP_ID=$(printf '%s\n' "$REMAP_OUTPUT" | awk '/^migration / {print $2}')
[ -n "$REMAP_ID" ] || fail "remap did not return a migration id"
run verify "$NEW" --old "$OLD" >/dev/null
[ "$(sqlite3 "$HOME_DIR/state_5.sqlite" "SELECT cwd FROM threads WHERE id='e2e-thread';")" = "$NEW" ] \
  || fail "remap did not update SQLite cwd"
run rollback "$REMAP_ID" --yes
[ "$(sqlite3 "$HOME_DIR/state_5.sqlite" "SELECT cwd FROM threads WHERE id='e2e-thread';")" = "$OLD" ] \
  || fail "remap rollback did not restore SQLite cwd"

rmdir "$NEW"
run plan "$OLD" "$NEW" --move-directory >/dev/null
MOVE_OUTPUT=$(run move "$OLD" "$NEW" --yes)
MOVE_ID=$(printf '%s\n' "$MOVE_OUTPUT" | awk '/^migration / {print $2}')
[ -n "$MOVE_ID" ] || fail "move did not return a migration id"
[ ! -e "$OLD" ] && [ -f "$NEW/marker.txt" ] || fail "project directory was not moved"
run verify "$NEW" --old "$OLD" >/dev/null
run rollback "$MOVE_ID" --yes
[ -f "$OLD/marker.txt" ] && [ ! -e "$NEW" ] || fail "move rollback did not restore directory"

echo "disposable e2e passed: inspect, plan, remap, verify, rollback, move, verify, rollback"
