#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
BIN=${CODEX_REHOME_BIN:-"$ROOT/target/release/codex-rehome"}
if [ -n "${CODEX_REHOME_TARGET:-}" ]; then
  HOST=$CODEX_REHOME_TARGET
else
  HOST=$(rustc -vV 2>/dev/null | sed -n 's/^host: //p' || true)
  if [ -z "$HOST" ]; then
    HOST=$(cargo -vV 2>/dev/null | sed -n 's/^host: //p' || true)
  fi
fi
[ -x "$BIN" ] || { echo "release binary not found: $BIN" >&2; exit 1; }
[ -n "$HOST" ] || { echo "could not determine Rust host target" >&2; exit 1; }
VERSION=$($BIN --version | awk '{print $2}')
NAME="codex-rehome-$VERSION-$HOST"
STAGE="$ROOT/target/package/$NAME"
DIST="$ROOT/dist"

rm -rf "$STAGE"
mkdir -p "$STAGE/completions" "$STAGE/man" "$DIST"
cp "$BIN" "$STAGE/codex-rehome"
cp "$ROOT/LICENSE" "$ROOT/README.md" "$STAGE/"
"$BIN" completions bash >"$STAGE/completions/codex-rehome.bash"
"$BIN" completions fish >"$STAGE/completions/codex-rehome.fish"
"$BIN" completions power-shell >"$STAGE/completions/_codex-rehome.ps1"
"$BIN" completions zsh >"$STAGE/completions/_codex-rehome"
"$BIN" manpage >"$STAGE/man/codex-rehome.1"

tar -C "$ROOT/target/package" -czf "$DIST/$NAME.tar.gz" "$NAME"
if command -v sha256sum >/dev/null 2>&1; then
  (cd "$DIST" && sha256sum "$NAME.tar.gz" >"$NAME.tar.gz.sha256")
else
  (cd "$DIST" && shasum -a 256 "$NAME.tar.gz" >"$NAME.tar.gz.sha256")
fi
echo "$DIST/$NAME.tar.gz"
