#!/usr/bin/env bash

set -euo pipefail

ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
cd "$ROOT"

PYTHON_BIN="${PYTHON:-python3}"

echo "▸ Checking Python interpreter..."
if ! command -v "$PYTHON_BIN" >/dev/null 2>&1; then
    cat <<EOF >&2
✖ Python 3 interpreter not found: $PYTHON_BIN

Install Python 3, then install the phase-3 dev dependencies:
  python3 -m pip install --requirement requirements-dev.txt
EOF
    exit 1
fi

missing_python_deps="$(
    "$PYTHON_BIN" - <<'PY'
import importlib.util

missing = []
for module_name, package_name in (("yaml", "PyYAML"), ("jsonschema", "jsonschema")):
    if importlib.util.find_spec(module_name) is None:
        missing.append(package_name)

print("\n".join(missing))
PY
)"

if [ -n "$missing_python_deps" ]; then
    cat <<EOF >&2
✖ Missing Python dev dependencies:
$missing_python_deps

Install them with:
  python3 -m pip install --requirement requirements-dev.txt
EOF
    exit 1
fi
echo "▸ Python imports OK: yaml, jsonschema"

echo "▸ Checking Chrome/Chromium availability..."
browser_candidates=(
    "google-chrome"
    "google-chrome-stable"
    "chromium"
    "chromium-browser"
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
    "/Applications/Chromium.app/Contents/MacOS/Chromium"
    "/c/Program Files/Google/Chrome/Application/chrome.exe"
    "/c/Program Files/Chromium/Application/chrome.exe"
    "/mnt/c/Program Files/Google/Chrome/Application/chrome.exe"
    "/mnt/c/Program Files/Chromium/Application/chrome.exe"
)

browser_path=""
for candidate in "${browser_candidates[@]}"; do
    if command -v "$candidate" >/dev/null 2>&1; then
        browser_path="$(command -v "$candidate")"
        break
    fi

    if [ -x "$candidate" ]; then
        browser_path="$candidate"
        break
    fi
done

if [ -z "$browser_path" ]; then
    cat <<'EOF' >&2
✖ Chrome/Chromium not found.

Install one of these binaries so Phase 3 MCP checks can lint real URLs:
  - google-chrome
  - google-chrome-stable
  - chromium
  - chromium-browser

Common install commands:
  Debian/Ubuntu: sudo apt-get install chromium-browser
  Fedora: sudo dnf install chromium
  macOS (Homebrew): brew install --cask google-chrome

Plumb does not install Chrome/Chromium for you.
EOF
    exit 1
fi

echo "▸ Browser found: $browser_path"
echo "▸ Phase 3 gate environment OK."
