# RogueAudio Architecture

Open-source Linux / Steam Deck audiobook client with first-class Plex integration.
Competes with Prologue’s feature set without cloning its UI.

## Goals

- Excellent Plex audiobook browsing, playback, and progress sync
- Clean, dark, listening-focused UI (touch + Steam Deck controller)
- Reliable sleep timer and progress reporting
- Offline downloads of self-hosted library content
- Fully open source — no DRM / no Audible

## Tech stack

| Layer | Choice | Notes |
|--------|--------|--------|
| Shell | Tauri 2 | Rust backend + webview frontend |
| UI | Svelte 5 + TypeScript + Tailwind | Lightweight, fast |
| State | Svelte stores | Simple, no extra runtime cost |
| Audio (MVP) | HTML5 Audio / Web Audio | Easy; swappable later |
| Audio (future) | libmpv or GStreamer | Better chapters / reliability |
| Packaging | Flatpak first | Steam Deck / Linux distribution |

## High-level system diagram

```
┌─────────────────────────────────────────────────────────────┐
│  Svelte UI (webview)                                        │
│  layout · library · player bar · auth · settings            │
│       │                                                     │
│       ├─ stores (auth, library, player, settings)           │
│       ├─ audio/ engine (HTML5)  ──position/events──┐        │
│       └─ api/ tauri invoke wrappers                 │        │
└───────────────────────┬─────────────────────────────┼────────┘
                        │ invoke / events             │
┌───────────────────────▼─────────────────────────────▼────────┐
│  Rust (src-tauri)                                            │
│  commands/*  →  plex/*  ·  storage/*  ·  downloads/*         │
│                     │                                        │
│                     ▼                                        │
│              Plex Media Server  ·  local SQLite / files      │
└──────────────────────────────────────────────────────────────┘
```

## Frontend structure

```
src/
├── app.css                 # Tailwind + dark theme tokens
├── app.html
├── lib/
│   ├── api/                # Thin invoke wrappers (no business UI)
│   │   ├── plex.ts
│   │   ├── library.ts
│   │   └── progress.ts
│   ├── audio/
│   │   └── engine.ts       # AudioEngine interface + Html5AudioEngine
│   ├── components/
│   │   ├── layout/         # AppShell, Nav, TopBar
│   │   ├── library/        # BookGrid, BookCard, SearchBar
│   │   ├── player/         # PlayerBar, SpeedControl, SleepTimer
│   │   └── auth/           # PlexPinLogin
│   ├── stores/
│   │   ├── auth.ts
│   │   ├── library.ts
│   │   ├── player.ts
│   │   └── settings.ts
│   └── types/
│       └── models.ts
└── routes/
    ├── +layout.svelte      # App shell + player bar
    ├── +page.svelte        # Library home
    ├── auth/+page.svelte
    └── settings/+page.svelte
```

### Audio abstraction

```ts
interface AudioEngine {
  load(url: string, headers?: Record<string, string>): Promise<void>;
  play(): Promise<void>;
  pause(): void;
  seek(seconds: number): void;
  setRate(rate: number): void;
  getPosition(): number;
  getDuration(): number;
  destroy(): void;
  // events: timeupdate, ended, error, playing, paused
}
```

MVP uses `Html5AudioEngine`. Later engines (mpv via Rust events, GStreamer) implement the same interface so the player store and UI stay stable.

## Rust structure

```
src-tauri/src/
├── main.rs
├── lib.rs                  # Builder, plugin registration, command table
├── commands/
│   ├── mod.rs
│   ├── plex.rs             # auth, servers, libraries, items, stream URL
│   ├── progress.rs         # report + fetch resume position
│   └── downloads.rs        # queue / status (stub → MVP)
├── plex/
│   ├── mod.rs
│   ├── auth.rs             # PIN flow + token persistence
│   ├── client.rs           # HTTP client (reqwest)
│   └── models.rs           # DTOs shared with frontend via serde
├── storage/
│   ├── mod.rs              # app data paths, secrets, SQLite later
│   └── config.rs
└── error.rs                # AppError → serializable for UI
```

### Command design principles

1. **Commands are the only UI boundary** — no ad-hoc HTTP from the frontend to Plex.
2. **Commands return serializable DTOs** — frontend never parses Plex XML/JSON shapes.
3. **Side effects live in Rust** — token storage, download paths, progress write-ahead.
4. **Idempotent where possible** — progress report can be retried safely.

#### Initial command surface

| Command | Purpose |
|---------|---------|
| `plex_start_pin_auth` | Create PIN; return `{ id, code, link }` |
| `plex_poll_pin_auth` | Poll until authorized; persist token |
| `plex_logout` | Clear token |
| `plex_auth_status` | `{ authenticated, username? }` |
| `plex_list_servers` | Discovered resources |
| `plex_list_libraries` | Music/audiobook sections |
| `plex_list_books` | Browse/search items in a library |
| `plex_get_stream` | Stream URL + headers for player |
| `progress_get` | Local + optional Plex resume |
| `progress_report` | Throttled write local + Plex timeline |
| `download_*` | Offline queue (later) |

## Data flow: playback & progress sync

```
User taps book
    → UI: library store selects book
    → invoke plex_get_stream(bookId)
    → Rust: resolve part key + X-Plex headers + token URL
    → UI: AudioEngine.load(url, headers)
    → invoke progress_get(bookId) → seek to resume
    → AudioEngine.play()

While playing (every ~10s, on pause, on seek, on sleep stop):
    → player store: position, duration, rate
    → invoke progress_report({
         ratingKey, state, timeMs, durationMs, speed
       })
    → Rust:
         1. Write local progress immediately (reliability)
         2. POST Plex :/timeline/  (best-effort + retry queue)
         3. Return ack + any server conflict

On app start / server switch:
    → reconcile local vs Plex (prefer most recent timestamp)
```

**Reliability rules**

- Local progress is source of truth if offline or Plex fails.
- Never lose a pause/seek/sleep-stop update: write local first.
- Debounce high-frequency `timeupdate`; always flush on lifecycle events.

## Sleep timer

Owned by the **player store** (frontend) for MVP:

- Modes: `off` | `duration` (minutes) | `end_of_chapter`
- Optional fade-out in last N seconds
- On fire: pause engine → `progress_report` → clear timer
- Rust is not required for timer math; chapters may later come from Rust metadata

## Offline downloads (post-foundation)

1. Rust downloads part files into app data with metadata JSON.
2. Library UI marks downloaded titles.
3. `plex_get_stream` returns `file://` or custom protocol when offline copy exists.
4. Progress still reports to Plex when online.

## MPRIS / background (Linux)

- After HTML5 path is stable: Rust `mpris` crate + media key handlers
- Commands/events: play/pause/seek from system → player store
- Flatpak needs appropriate portals / session bus access

## UI / UX principles

- Always dark by default (OLED / night listening)
- Large touch targets (≥ 44px), focus rings for gamepad
- Persistent bottom **player bar** on all routes
- Library-first home; auth is a gate, not the home forever
- Minimal chrome; cover art and transport dominate

## Plex client identity

| Header / field | Value |
|----------------|--------|
| Product | `RogueAudio` |
| Client identifier | `app.rogueaudio` |
| App / Flatpak id | `app.rogueaudio` |

Defined in `src-tauri/src/plex/identity.rs` — keep these stable so Plex authorized-device lists stay consistent across updates and platforms.

## Library filtering

- Audiobooks in Plex are **Music** sections (`type=artist`).
- `plex_list_libraries` returns **only** music-type libraries.
- If more than one music library exists (e.g. “Audiobooks” + “Music”), the UI shows a **Library** filter and defaults to titles matching audio/book/spoken.

## Cross-platform notes (Linux now, Windows later)

Tauri already targets Linux / Windows / macOS from one codebase. When adding Windows:

| Area | Notes |
|------|--------|
| Paths | Use `dirs` crate (already) — not hard-coded `~/.local/share` |
| MPRIS | Linux-only; gate with `#[cfg(target_os = "linux")]`; Windows uses SMTC later |
| Flatpak | Linux packaging only; Windows uses MSI/NSIS via Tauri bundler |
| Webview | WebKitGTK (Linux) vs WebView2 (Windows) — HTML5 audio should work on both; test codecs (m4b/m4a) |
| Media keys | Abstract behind a `system_media` module so OS backends swap cleanly |
| Plex identity | Same `app.rogueaudio` client id on all OS builds |

No major frontend refactor expected for Windows; backend system integrations stay behind cfg / trait boundaries.

## Packaging

1. Develop with `npm run tauri dev`
2. Release: Tauri bundle → Flatpak (primary for Deck); Windows later via Tauri
3. Identifier: `app.rogueaudio`

## Implementation phases

1. **Foundation** (current) — scaffold, theme, shell, command stubs, audio abstraction
2. **Plex auth + library list** — real PIN flow and browse
3. **Player MVP** — play/pause, speed, sleep timer against stream URL
4. **Progress sync** — local + Plex timeline
5. **Chapters** — metadata + end-of-chapter sleep
6. **Downloads + MPRIS + Flatpak polish**

## Constraints (non-negotiable)

- Open source only
- No DRM / Audible
- Modular UI ↔ Rust boundary
- Progress sync and sleep timer reliability over flashy features
