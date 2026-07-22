# Developing RogueAudio on Steam Deck

SteamOS keeps a read-only root and does not ship Tauri’s system libraries
(`webkit2gtk`, `gcc`, etc.). We use a **Distrobox** container so you can build
and run the native app without modifying SteamOS.

## One-time setup (already done on this machine)

Box name: **`rogue-dev`** (Ubuntu 24.04)

```bash
# Recreate if needed
distrobox create --name rogue-dev --image docker.io/library/ubuntu:24.04 --yes

distrobox enter rogue-dev -- bash -lc '
  export DEBIAN_FRONTEND=noninteractive
  sudo apt-get update -y
  sudo apt-get install -y \
    libwebkit2gtk-4.1-dev build-essential curl wget file \
    libxdo-dev libssl-dev libayatana-appindicator3-dev \
    librsvg2-dev pkg-config libgtk-3-dev patchelf ca-certificates git
'
```

Node and Rust live in your **home directory** (`~/.nvm`, `~/.cargo`) and are
shared with the box. Install them on the host once if missing.

## Day-to-day commands

### Full Tauri app (native window)

```bash
cd ~/Dev/GROK/RogueAudio
./scripts/dev-tauri.sh
# or:
distrobox enter rogue-dev
# then inside the box:
cd ~/Dev/GROK/RogueAudio
npm run tauri:dev
```

### Frontend only (no native shell)

Works on SteamOS host without Distrobox:

```bash
cd ~/Dev/GROK/RogueAudio
npm run dev
# http://localhost:1420
```

## Notes

- Project files are the same on host and in the box (`~/` is shared).
- First `cargo` / `tauri dev` compile is slow; later builds are incremental.
- GUI apps need a working `DISPLAY` (Desktop Mode on Deck is fine).
- To remove the box later: `distrobox rm rogue-dev`
