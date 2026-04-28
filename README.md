# AI Shepherd

AI Shepherd is a local desktop companion for AI coding tools. It is built with **Tauri 2**, **Vue 3**, **TypeScript**, and **Rust**.

Current features:

- Windows-first Tauri desktop shell with system tray.
- WezTerm pane discovery/focus/send-text foundation.
- Claude Code usage monitor reading local JSONL session files.
- Per-session Claude usage summaries with labels from `custom-title`, `ai-title`, or the first user message.

The app does not send captured data to a remote service.

## Requirements

- Node.js compatible with the current frontend toolchain.
- pnpm `10.28.2` or compatible.
- Rust toolchain.
- Tauri 2 system requirements for your OS.
- Optional: WezTerm in `PATH` for terminal pane integration.
- Optional: Claude Code local logs under `~/.claude/projects/**/*.jsonl` for usage summaries.

On Windows, WebView2 is required by Tauri.

## Install

```bash
pnpm install
```

## Run in development

```bash
pnpm tauri dev
```

The Vue dev server uses:

```txt
http://localhost:6001
```

## Build

```bash
pnpm tauri build
```

For a debug Tauri build:

```bash
pnpm tauri build --debug
```

## Useful checks

Frontend:

```bash
pnpm build
```

Rust backend:

```bash
cd src-tauri
cargo check
cargo clippy -- -D warnings
```

## Notes

- Clicking the window close button hides the app to the tray.
- Use tray `Show` to restore the window.
- Use tray `Quit` to exit.
- On Windows, WebView2 may still print a shutdown warning like `Chrome_WidgetWin_0`. If the process exits successfully, this is treated as non-blocking WebView2 noise.

## Project structure

```txt
src/                 Vue frontend
src-tauri/           Rust/Tauri backend
src-tauri/src/usage.rs      Claude usage parsing and summaries
src-tauri/src/terminal.rs   Terminal adapter foundation
documentation/       Technical specification
```
