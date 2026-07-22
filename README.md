# RogueAudio

Open-source **Linux** and **Steam Deck** audiobook client with first-class **Plex** integration.

Built to compete with Prologue’s feature set (not a visual clone): clean dark UI, reliable progress sync, sleep timer, offline downloads, and controller-friendly navigation — all for **self-hosted** libraries. No DRM. No Audible.

## Stack

| Layer | Tech |
|--------|------|
| App shell | [Tauri 2](https://tauri.app/) (Rust + webview) |
| UI | Svelte 5 + TypeScript + Tailwind CSS |
| State | Svelte stores |
| Audio (MVP) | HTML5 Audio (swappable → libmpv / GStreamer later) |
| Packaging | Flatpak-first (planned) |

## Architecture

See [docs/ARCHITECTURE.md](./docs/ARCHITECTURE.md) for:

- Frontend / Rust module layout
- Tauri command surface
- Playback + progress sync data flow
- Sleep timer design
- Implementation phases

## Status (foundation)

- [x] Tauri 2 + SvelteKit + TypeScript scaffold
- [x] Dark listening-focused UI shell (nav + library + player bar)
- [x] Modular Rust commands (Plex, progress, downloads stubs)
- [x] Audio engine abstraction + demo playback path
- [x] Sleep timer (duration) + speed control UI
- [x] Local progress persistence (JSON; Plex timeline next)
- [ ] Live Plex PIN auth + library fetch
- [ ] Real stream playback
- [ ] Chapters, downloads, MPRIS, Flatpak

## Development

### Prerequisites

- Node.js 20+
- Rust (stable) via [rustup](https://rustup.rs/)
- Linux Tauri deps: [Tauri Linux prerequisites](https://v2.tauri.app/start/prerequisites/)  
  (webkit2gtk, rsvg2, and a C toolchain)

On SteamOS / Steam Deck you typically need a writable root or distrobox/toolbox with those libraries to run `tauri dev`.

### Setup

```bash
cd RogueAudio
npm install
```

### Frontend only (no native window)

Useful for UI work without full Tauri system deps:

```bash
npm run dev
```

Open http://localhost:1420 — Rust `invoke` calls need the Tauri runtime; use the **stub auth** button to explore UI when running inside `tauri dev`.

### Full app

```bash
npm run tauri:dev
```

### Build

```bash
npm run tauri:build
```

## Quick UI tour

1. **Plex** page → *Continue with stub auth* (dev) or PIN flow (live, upcoming)
2. **Library** → sample books (stub) or your Plex libraries
3. Tap a book → player bar loads; play/pause, ±30s, speed, sleep timer
4. Progress is written under `~/.local/share/rogue-audio/progress/`

## License

MIT
