# Project structure

```
RogueAudio/
├── docs/
│   ├── ARCHITECTURE.md
│   └── PROJECT_STRUCTURE.md
├── src/                          # SvelteKit frontend (SPA via adapter-static)
│   ├── app.css
│   ├── app.html
│   ├── app.d.ts
│   ├── lib/
│   │   ├── api/
│   │   ├── audio/
│   │   ├── components/
│   │   ├── stores/
│   │   └── types/
│   └── routes/
├── src-tauri/                    # Rust / Tauri backend
│   ├── capabilities/
│   ├── icons/
│   ├── src/
│   │   ├── commands/
│   │   ├── plex/
│   │   ├── storage/
│   │   ├── error.rs
│   │   ├── lib.rs
│   │   └── main.rs
│   ├── Cargo.toml
│   └── tauri.conf.json
├── static/
├── package.json
├── svelte.config.js
├── vite.config.js
├── tsconfig.json
└── README.md
```
