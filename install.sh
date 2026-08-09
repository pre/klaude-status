#!/bin/sh
# Build klaude-status, install it, and point Claude Code's statusLine at it.
#
#   ./install.sh              # installs into ~/.local/bin
#   PREFIX=/usr/local ./install.sh   # installs into /usr/local/bin
#
# The statusLine command is written as an absolute path on purpose: Claude Code
# runs it without your shell profile, so a bare name resolves in a terminal
# session but not in the desktop app, and the only symptom is an empty line.
set -eu
cd "$(dirname "$0")"

BIN_DIR="${PREFIX:+$PREFIX/bin}"
BIN_DIR="${BIN_DIR:-$HOME/.local/bin}"
TARGET="$BIN_DIR/klaude-status"

command -v cargo >/dev/null 2>&1 || {
    echo "error: cargo not found - install Rust via https://rustup.rs" >&2
    exit 1
}

cargo build --release --locked

mkdir -p "$BIN_DIR"
# Copy to a temporary name and rename: the status line may be running right
# now, and rename is atomic where overwriting in place is not.
cp target/release/klaude-status "$TARGET.tmp.$$"
chmod 755 "$TARGET.tmp.$$"
mv -f "$TARGET.tmp.$$" "$TARGET"

echo
echo "installed: $TARGET"

case ":$PATH:" in
    *":$BIN_DIR:"*) ;;
    *) echo "note: $BIN_DIR is not on your PATH" >&2 ;;
esac

SETTINGS="$HOME/.claude/settings.json"
if command -v python3 >/dev/null 2>&1 && [ -f "$SETTINGS" ]; then
    TARGET="$TARGET" python3 - "$SETTINGS" <<'PY'
import json, os, sys

path, target = sys.argv[1], os.environ["TARGET"]
try:
    settings = json.load(open(path))
except Exception as err:
    sys.exit(f"note: cannot read {path} ({err}); set statusLine by hand")

current = settings.get("statusLine") or {}
command = current.get("command", "")
# Leave a status line alone if it points at something else entirely.
if command and "klaude-status" not in command:
    print(f'note: statusLine is already "{command}"; leaving it alone')
    sys.exit(0)
if command == target:
    print(f"statusLine: {target} (already set)")
    sys.exit(0)

settings["statusLine"] = {
    "type": "command",
    "command": target,
    "refreshInterval": current.get("refreshInterval", 10),
}
tmp = path + ".tmp"
with open(tmp, "w") as handle:
    json.dump(settings, handle, indent=2, ensure_ascii=False)
os.replace(tmp, path)
print(f"statusLine: {command or '(nothing)'} -> {target}")
PY
else
    echo
    echo "add this to $SETTINGS:"
    echo "  \"statusLine\": { \"type\": \"command\", \"command\": \"$TARGET\", \"refreshInterval\": 10 }"
fi
