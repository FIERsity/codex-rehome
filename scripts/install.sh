#!/bin/sh
set -eu

VERSION=${CODEX_REHOME_VERSION:-0.1.0-alpha.2}
PREFIX=${CODEX_REHOME_PREFIX:-"$HOME/.local"}
REPOSITORY=${CODEX_REHOME_REPOSITORY:-FIERsity/codex-rehome}

case "$(uname -s):$(uname -m)" in
  Darwin:arm64) TARGET=aarch64-apple-darwin ;;
  Darwin:x86_64) TARGET=x86_64-apple-darwin ;;
  Linux:x86_64) TARGET=x86_64-unknown-linux-gnu ;;
  *) echo "unsupported platform: $(uname -s) $(uname -m)" >&2; exit 1 ;;
esac

NAME="codex-rehome-$VERSION-$TARGET"
BASE="https://github.com/$REPOSITORY/releases/download/v$VERSION"
TMP=$(mktemp -d "${TMPDIR:-/tmp}/codex-rehome-install.XXXXXX")
trap 'rm -rf "$TMP"' EXIT HUP INT TERM

curl --fail --location --proto '=https' --tlsv1.2 \
  --output "$TMP/$NAME.tar.gz" "$BASE/$NAME.tar.gz"
curl --fail --location --proto '=https' --tlsv1.2 \
  --output "$TMP/$NAME.tar.gz.sha256" "$BASE/$NAME.tar.gz.sha256"
if command -v sha256sum >/dev/null 2>&1; then
  (cd "$TMP" && sha256sum --check "$NAME.tar.gz.sha256")
else
  (cd "$TMP" && shasum -a 256 --check "$NAME.tar.gz.sha256")
fi
tar -C "$TMP" -xzf "$TMP/$NAME.tar.gz"

mkdir -p \
  "$PREFIX/bin" \
  "$PREFIX/share/man/man1" \
  "$PREFIX/share/bash-completion/completions" \
  "$PREFIX/share/fish/vendor_completions.d" \
  "$PREFIX/share/zsh/site-functions"
install -m 755 "$TMP/$NAME/codex-rehome" "$PREFIX/bin/codex-rehome"
install -m 644 "$TMP/$NAME/man/codex-rehome.1" "$PREFIX/share/man/man1/codex-rehome.1"
install -m 644 "$TMP/$NAME/completions/codex-rehome.bash" "$PREFIX/share/bash-completion/completions/codex-rehome"
install -m 644 "$TMP/$NAME/completions/codex-rehome.fish" "$PREFIX/share/fish/vendor_completions.d/codex-rehome.fish"
install -m 644 "$TMP/$NAME/completions/_codex-rehome" "$PREFIX/share/zsh/site-functions/_codex-rehome"

echo "installed codex-rehome $VERSION to $PREFIX/bin/codex-rehome"
case ":$PATH:" in
  *":$PREFIX/bin:"*) ;;
  *) echo "add $PREFIX/bin to PATH before running codex-rehome" ;;
esac
