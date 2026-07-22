#!/usr/bin/env bash
# Run Tauri dev inside the rogue-dev Distrobox (Steam Deck / host without system deps).
set -euo pipefail

BOX="${DISTROBOX_NAME:-rogue-dev}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"

if ! command -v distrobox >/dev/null 2>&1; then
  echo "distrobox not found. Install it or run on a machine with Tauri system deps." >&2
  exit 1
fi

if ! distrobox list 2>/dev/null | grep -q "$BOX"; then
  echo "Distrobox '$BOX' not found." >&2
  echo "Create it with the steps in docs/DEV_STEAM_DECK.md" >&2
  exit 1
fi

exec distrobox enter "$BOX" -- bash -lc "
  set -e
  export NVM_DIR=\"\$HOME/.nvm\"
  [ -s \"\$NVM_DIR/nvm.sh\" ] && . \"\$NVM_DIR/nvm.sh\"
  [ -f \"\$HOME/.cargo/env\" ] && . \"\$HOME/.cargo/env\"
  export PATH=\"\$HOME/.local/bin:\$PATH\"
  cd \"$ROOT\"
  npm run tauri:dev
"
