# AGENTS.md

## Project overview

AI Shepherd is a local Tauri 2 + Vue 3 + TypeScript + Rust desktop app for monitoring AI coding tool usage and terminal sessions.

## Development commands

- Install dependencies: `pnpm install`
- Run app in development: `pnpm tauri dev`
- Build frontend: `pnpm build`
- Build Tauri app: `pnpm tauri build`
- Rust checks:
  - `cd src-tauri && cargo check`
  - `cd src-tauri && cargo clippy -- -D warnings`

## Notes for agents

- The Vue dev server uses port `6001`; do not switch to `6000` because Chromium/WebView2 blocks it as an unsafe port.
- Keep line endings as LF.
- Do not commit generated build outputs such as `dist/`, `target/`, or `src-tauri/gen/`.
- Claude Code usage parsing lives in `src-tauri/src/usage.rs`.
- Terminal integration lives in `src-tauri/src/terminal.rs`.
- The technical specification is in `documentation/initial-spec.md`.
