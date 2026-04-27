# AI Shepherd — Technical Specification

**Version**: 0.2-draft
**Stack**: Tauri 2 · Vue 3 · TypeScript · WezTerm CLI
**Primary target**: Windows (WSL2 + PowerShell)
**Secondary targets**: macOS, Linux (architecture planned, not tested in v1)

---

## 1. Overview

AI Shepherd is a lightweight desktop application that lives in the system tray. It serves two main purposes:

1. **Usage Monitor** — real-time tracking of AI coding tool session consumption (Claude Code, Codex, Gemini CLI), with quota alerts and history.
2. **Pending Prompts Viewer** — detection of terminal panes waiting for a user response, with context display in Markdown.

The architecture is designed so that adding a new terminal (Windows Terminal, Alacritty, kitty…) or a new AI provider mostly requires writing a single adapter, without touching the application core. Cross-platform support (macOS, Linux) is planned from the design phase: OS-specific runtime code lives in dedicated modules activated by Rust feature flags (`#[cfg(target_os = "...")]`), while the core stays OS-agnostic.

---

## 2. General architecture

```
┌──────────────────────────────────────────────────────┐
│                     Tauri App                        │
│                                                      │
│  ┌─────────────────┐      ┌──────────────────────┐  │
│  │   Vue 3 Frontend │◄────►│   Rust Backend       │  │
│  │   (WebView)      │      │   (Core)             │  │
│  │                  │      │                      │  │
│  │  - UsagePanel    │      │  - TerminalRegistry  │  │
│  │  - PendingPanel  │      │  - ProviderRegistry  │  │
│  │  - SettingsPanel │      │  - FileWatcher       │  │
│  │  - TrayMenu      │      │  - EventBus          │  │
│  └─────────────────┘      └──────────┬───────────┘  │
└─────────────────────────────────────┼───────────────┘
                                       │
              ┌────────────────────────┼──────────────────┐
              │                        │                   │
   ┌──────────▼──────┐    ┌────────────▼──────┐  ┌───────▼──────┐
   │ TerminalAdapter │    │  ProviderAdapter  │  │  FileAdapter │
   │  (trait)        │    │  (trait)          │  │  (trait)     │
   └────────┬────────┘    └────────┬──────────┘  └──────┬───────┘
            │                      │                     │
   ┌────────▼────────┐    ┌────────▼───────┐   ┌────────▼───────┐
   │  WezTermAdapter │    │ ClaudeProvider │   │ JsonlWatcher   │
   │  (impl)         │    │ CodexProvider  │   │ (impl)         │
   │                 │    │ GeminiProvider │   │                │
   │  [future]       │    │ [extensible]   │   │                │
   │  WinTermAdapter │    └────────────────┘   └────────────────┘
   │  KittyAdapter   │
   └─────────────────┘
```

### 2.1 Extensibility principles

Each external integration (terminal, AI provider) is isolated behind a **Rust trait**. The central application only knows the traits, never the concrete implementations. Adapters are registered at startup in registries (`TerminalRegistry`, `ProviderRegistry`).

Adding a terminal = implementing `TerminalAdapter` plus declaring which capabilities it actually supports. Features such as prompt viewing, focus, and text sending are enabled only when the adapter exposes the required capability.
Adding a provider = implementing `ProviderAdapter` + `UsageFileAdapter`.

### 2.2 Cross-platform principle

All OS-specific code is contained in only two places:

- **`src/platform/`** — Rust module dedicated to OS abstractions and platform-specific runtime behavior (paths, tray, WSL detection)
- **Terminal adapters** — which naturally encapsulate the specifics of each terminal

The application core (`TerminalRegistry`, `ProviderRegistry`, `FileWatcher`, `EventBus`) contains no `#[cfg(target_os)]`.

---

## 3. Traits and interfaces

### 3.1 `TerminalAdapter` (Rust trait)

Unchanged regardless of the target OS, but adapters are best-effort: future terminals may expose only a subset of capabilities.

```rust
/// A terminal adapter exposes active panes and allows sending text to them.
pub trait TerminalAdapter: Send + Sync {
    /// Unique identifier of this adapter (e.g.: "wezterm", "windows-terminal")
    fn id(&self) -> &'static str;

    /// Human-readable name for the UI (e.g.: "WezTerm", "Windows Terminal")
    fn display_name(&self) -> &'static str;

    /// OSes supported by this adapter
    fn supported_platforms(&self) -> &[Platform];

    /// Verifies that the terminal is available on this system
    fn is_available(&self) -> bool;

    /// Returns the list of all open panes if supported.
    fn list_panes(&self) -> Result<Vec<PaneInfo>, AdapterError>;

    /// Capability flags for UI/command availability.
    fn capabilities(&self) -> TerminalCapabilities;

    /// Sends text to a pane using a shell-safe mechanism.
    fn send_text(&self, pane_id: &str, text: &str) -> Result<(), AdapterError>;

    /// Captures the visible content of a pane if supported.
    fn capture_pane(&self, pane_id: &str) -> Result<String, AdapterError>;

    /// Focuses/activates a pane if supported.
    fn focus_pane(&self, pane_id: &str) -> Result<(), AdapterError>;
}

pub struct TerminalCapabilities {
    pub list_panes: bool,
    pub capture_pane: bool,
    pub send_text: bool,
    pub focus_pane: bool,
}

pub enum Platform {
    Windows,
    MacOs,
    Linux,
}
```

### 3.2 `PaneInfo` (shared structure)

```rust
pub struct PaneInfo {
    pub id: String,                   // opaque identifier (e.g.: "3" for wezterm pane_id)
    pub terminal_adapter: String,     // e.g.: "wezterm"
    pub title: String,                // pane title
    pub cwd: Option<CwdLocation>,     // current directory in terminal-native form plus parsed context
    pub shell: ShellType,             // PowerShell, Bash, Zsh, Fish, Unknown
    pub context: ShellContext,        // Native | Wsl { distro } | Ssh { host }
    pub process_name: Option<String>, // foreground process
    pub last_seen: SystemTime,
}

pub enum ShellType {
    PowerShell,
    Bash,
    Zsh,
    Fish,
    Cmd,
    Unknown(String),
}

/// Shell execution context within the pane.
/// On Windows: Native (PowerShell/Cmd) or Wsl.
/// On macOS/Linux: always Native (Wsl does not exist).
pub enum ShellContext {
    Native,
    Wsl { distro: String },  // Windows only — never instantiated elsewhere
    Ssh { host: String },    // future
}

pub struct CwdLocation {
    pub raw: String,          // original file:// URI or terminal-native path string
    pub normalized: Option<String>, // parsed/normalized path when safe and unambiguous
}
```

### 3.3 `ProviderAdapter` (Rust trait)

```rust
pub trait ProviderAdapter: Send + Sync {
    fn id(&self) -> &'static str;
    fn display_name(&self) -> &'static str;
    fn matches_pane(&self, pane: &PaneInfo) -> bool;
    fn detect_pending(&self, pane_content: &str) -> Option<PendingPrompt>;
}
```

### 3.4 `UsageFileAdapter` (Rust trait)

```rust
pub trait UsageFileAdapter: Send + Sync {
    fn provider_id(&self) -> &'static str;
    fn watch_descriptors(&self) -> Vec<WatchDescriptor>;
    fn parse_file(&self, path: &Path) -> Result<Vec<UsageEvent>, AdapterError>;
    fn current_session_summary(&self) -> Result<SessionSummary, AdapterError>;
}

pub struct WatchDescriptor {
    pub roots: Vec<PathBuf>,
    pub includes: Vec<String>,
    pub excludes: Vec<String>,
}
```

---

## 4. Platform module — OS abstractions

This module is the **only place** allowed to contain conditional `#[cfg(target_os)]` code. Everything else in the application calls it without knowing which OS it runs on.

### 4.1 `PathResolver`

```rust
pub struct PathResolver;

impl PathResolver {
    /// Application configuration directory.
    ///
    /// Windows  → C:\Users\<user>\AppData\Roaming\ai-shepherd\
    /// macOS    → ~/Library/Application Support/ai-shepherd/
    /// Linux    → ~/.config/ai-shepherd/
    pub fn config_dir() -> PathBuf {
        dirs::config_dir().expect("config dir").join("ai-shepherd")
    }

    /// Data directory (SQLite, cache).
    ///
    /// Windows  → C:\Users\<user>\AppData\Local\ai-shepherd\
    /// macOS    → ~/Library/Application Support/ai-shepherd/
    /// Linux    → ~/.local/share/ai-shepherd/
    pub fn data_dir() -> PathBuf {
        dirs::data_local_dir().expect("data dir").join("ai-shepherd")
    }

    /// Resolves a path relative to the current user's home.
    /// On all OSes, uses dirs::home_dir().
    /// On Windows, also handles the WSL home if `context` == Wsl.
    pub fn resolve_home(relative: &str, context: &ShellContext) -> PathBuf {
        #[cfg(target_os = "windows")]
        if let ShellContext::Wsl { distro } = context {
            return Self::wsl_home(distro).join(relative);
        }

        dirs::home_dir().expect("home dir").join(relative)
    }

    /// Returns all candidate paths for a relative path.
    /// On Windows: native home + all detected WSL homes.
    /// On macOS/Linux: native home only.
    pub fn all_candidates(relative: &str) -> Vec<PathBuf> {
        let mut paths = vec![
            dirs::home_dir().expect("home dir").join(relative)
        ];

        #[cfg(target_os = "windows")]
        paths.extend(Self::wsl_candidates(relative));

        paths
    }

    /// Windows only: lists installed WSL distros and returns
    /// their UNC paths \\wsl$\<distro>\home\<user>\<relative>
    #[cfg(target_os = "windows")]
    fn wsl_candidates(relative: &str) -> Vec<PathBuf> {
        // wsl.exe --list --quiet → list of distros
        // For each distro: \\wsl$\<distro>\home\<user>\<relative>
        todo!()
    }

    #[cfg(target_os = "windows")]
    fn wsl_home(distro: &str) -> PathBuf {
        // \\wsl$\<distro>\home\<username>
        // username retrieved via: wsl.exe -d <distro> -- whoami
        todo!()
    }
}
```

### 4.2 `TrayBackend`

Tray behavior is unified behind Tauri, but OS constraints differ:

```
Windows  → WebView2, native Win32 tray — no constraint
macOS    → Menu bar app, native tray — ideal for this type of app
Linux    → Requires libappindicator or StatusNotifierItem
           Vanilla GNOME: no tray without extension
           KDE / XFCE / i3 / Sway: native tray
```

The app uses the Tauri 2 tray API for tray UI; `tauri-plugin-shell` is reserved for spawning external commands if needed. On Linux, if the tray is unavailable, the app falls back to a visible main window at startup (not minimized). This behavior is detected at runtime via tray initialization, and the fallback should be explicit rather than silent.

### 4.3 `NotificationBackend`

`tauri-plugin-notification` handles all three OSes transparently:

```
Windows  → Toast notifications (Action Center)
macOS    → Notification Center
Linux    → libnotify / D-Bus
```

No OS-specific code required.

---

## 5. WezTerm implementation

Since WezTerm is available on Windows, macOS and Linux with an identical CLI, the adapter is **100% cross-platform** without any `#[cfg]`.

### 5.1 `WezTermAdapter`

```rust
pub struct WezTermAdapter {
    binary_path: PathBuf, // detected via which::which("wezterm")
}

impl TerminalAdapter for WezTermAdapter {
    fn id(&self) -> &'static str { "wezterm" }
    fn display_name(&self) -> &'static str { "WezTerm" }

    fn supported_platforms(&self) -> &[Platform] {
        &[Platform::Windows, Platform::MacOs, Platform::Linux]
    }

    fn is_available(&self) -> bool {
        which::which("wezterm").is_ok()
    }

    fn list_panes(&self) -> Result<Vec<PaneInfo>, AdapterError> {
        // wezterm cli list --format json
        // → parse Vec<WezPaneRaw> → Vec<PaneInfo>
        // WSL detection is done in detect_context() below
    }

    fn send_text(&self, pane_id: &str, text: &str) -> Result<(), AdapterError> {
        // Spawn the CLI directly without a shell; write text to stdin
        // or pass safe CLI args. Never use shell piping/echo here.
    }

    fn capture_pane(&self, pane_id: &str) -> Result<String, AdapterError> {
        // wezterm cli get-text --pane-id {id}
        // + strip_ansi_escapes::strip()
    }

    fn focus_pane(&self, pane_id: &str) -> Result<(), AdapterError> {
        // wezterm cli activate-pane --pane-id {id}
        // Spawn directly without shell interpolation.
    }
}

impl WezTermAdapter {
    /// Detects the shell context of a pane from adapter metadata, explicit WSL path parsing,
    /// and terminal-native cwd information. Do not default silently to a distro name.
    fn detect_context(raw: &WezPaneRaw) -> ShellContext {
        let cwd = raw.cwd.as_deref().unwrap_or("");

        if let Some(distro) = Self::extract_wsl_distro(cwd) {
            return ShellContext::Wsl { distro };
        }

        ShellContext::Native
    }

    fn extract_wsl_distro(cwd: &str) -> Option<String> {
        // "file:///wsl%24/Ubuntu/home/..." → "Ubuntu"
        // "\\wsl$\Ubuntu\..." → "Ubuntu"
        todo!()
    }
}
```

### 5.2 Raw JSON `wezterm cli list`

Identical on all three OSes:

```json
[
  {
    "window_id": 0,
    "tab_id": 0,
    "pane_id": 3,
    "workspace": "default",
    "size": { "rows": 48, "cols": 220 },
    "title": "claude -- user@machine:~/project",
    "cwd": "file:///home/user/project"
  }
]
```

---

## 6. AI Providers — paths by OS

Providers use `PathResolver::all_candidates()` to find their log files. The table below shows the automatically resolved paths:

### 6.1 Claude Code

| OS | Resolved path |
|----|--------------|
| Windows (native) | `C:\Users\<user>\.claude\projects\**\*.jsonl` |
| Windows (WSL2) | `\\wsl$\<distro>\home\<user>\.claude\projects\**\*.jsonl` |
| macOS | `~/.claude/projects/**/*.jsonl` |
| Linux | `~/.claude/projects/**/*.jsonl` |

**Pending detection** — identical on all OSes:
- Process name contains `claude` or pane title contains `claude`
- Last line ends with `(y/n)`, `[Y/n]`, `[y/N]`, or cursor `▌` / `❯ `

### 6.2 Codex CLI

| OS | Resolved path |
|----|--------------|
| Windows (native) | `C:\Users\<user>\.codex\logs\*.jsonl` |
| Windows (WSL2) | `\\wsl$\<distro>\home\<user>\.codex\**` |
| macOS / Linux | `~/.codex/logs/*.jsonl` |

### 6.3 Gemini CLI

| OS | Resolved path |
|----|--------------|
| Windows (native) | `%APPDATA%\gemini\` → via `dirs::config_dir()` |
| macOS | `~/Library/Application Support/gemini/` |
| Linux | `~/.config/gemini/` |

---

## 7. Data models

### 7.1 Usage & session

```rust
pub struct UsageEvent {
    pub timestamp: DateTime<Utc>,
    pub provider_id: String,
    pub model: Option<String>,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cost_usd: Option<f64>,
    pub session_id: Option<String>,
}

pub struct SessionSummary {
    pub provider_id: String,
    pub provider_display: String,
    pub account_type: AccountType,
    pub tokens_used: u64,
    pub tokens_limit: Option<u64>,
    pub requests_used: u32,
    pub requests_limit: Option<u32>,
    pub cost_usd: f64,
    pub reset_at: Option<DateTime<Utc>>,
    pub burn_rate_per_hour: Option<f64>,
    pub period: UsagePeriod, // grouped by provider/account/workspace/session/period as applicable
}

pub enum AccountType {
    Subscription { plan: String },
    ApiUsageBased,
    Free,
    Unknown,
}
```

### 7.2 Pending prompts

```rust
pub struct PendingPrompt {
    pub pane: PaneInfo,
    pub provider_id: String,
    pub provider_display: String,
    pub detected_at: DateTime<Utc>,
    pub raw_content: String,
    pub markdown_content: String,
    pub prompt_type: PromptType,
    pub urgency: Urgency,
}

pub enum PromptType {
    Confirmation { question: String },
    FreeInput { context: String },
    DiffApproval { file_path: Option<String> },
    Other,
}

pub enum Urgency { Low, Medium, High }
```

---

## 8. Feature 1 — Usage Monitor

### 8.1 Behavior

At startup, the `FileWatcher` uses `notify` for Windows native paths. WSL UNC paths are not assumed reliable: they require polling or a WSL-side helper/agent unless proven reliable in practice.

On each modification:
1. The adapter parses the new lines
2. Events are aggregated in `UsageStore`
3. Tauri event emitted → Vue frontend: `usage:updated`

Ingestion must be incremental: store per-file offsets, handle truncation/rotation, deduplicate repeated records, and tolerate partially written lines until a complete record is available.

### 8.2 Calculations

```
burn_rate          = tokens_in_the_last_hour / 1h
time_to_limit      = (tokens_limit - tokens_used) / burn_rate
percent_used       = tokens_used / tokens_limit * 100
```

### 8.3 Alerts

Configurable thresholds (default: 75%, 90%, 100%) via `tauri-plugin-notification` — cross-platform without specific code.

### 8.4 SQLite persistence

```
Windows  → C:\Users\<user>\AppData\Local\ai-shepherd\usage.db
macOS    → ~/Library/Application Support/ai-shepherd/usage.db
Linux    → ~/.local/share/ai-shepherd/usage.db
```

Path resolved via `PathResolver::data_dir()`.

```sql
CREATE TABLE usage_events (
    id            INTEGER PRIMARY KEY,
    timestamp     TEXT NOT NULL,
    provider_id   TEXT NOT NULL,
    model         TEXT,
    input_tokens  INTEGER,
    output_tokens INTEGER,
    cost_usd      REAL,
    session_id    TEXT
);
```

---

## 9. Feature 2 — Pending Prompts Viewer

### 9.1 Behavior

`PendingPoller` polls each pane every 2s (configurable):

1. Retrieves `PaneInfo` via the terminal adapter
2. Checks if a provider `matches_pane`
3. If so, `capture_pane` + `detect_pending`
4. Emits `pending:detected` or `pending:resolved`

### 9.2 Terminal → Markdown conversion

```rust
fn raw_to_markdown(raw: &str, prompt_type: &PromptType) -> String {
    // 1. strip-ansi-escapes
    // 2. Preserve existing Markdown where possible; escape literal characters that would change structure
    // 3. Code block detection
    // 4. List detection
    // 5. Diff preservation (→ ```diff```) 
    // 6. Returns valid Markdown without losing raw text meaning
}
```

### 9.3 Display

Each pending prompt is displayed as a card:
- Provider icon + name
- Pane title + context (Native / WSL `<distro>` / SSH)
- Timestamp
- Scrollable Markdown content
- Type badge + urgency badge
- **"Focus pane"** button → enabled only when `TerminalCapabilities.focus_pane` is true; calls the selected adapter's `focus_pane` implementation (WezTerm uses `wezterm cli activate-pane --pane-id X`).

---

## 10. Vue 3 frontend architecture

### 10.1 Component structure

```
src/
├── App.vue
├── components/
│   ├── usage/
│   │   ├── UsagePanel.vue
│   │   ├── ProviderCard.vue
│   │   ├── UsageBar.vue
│   │   └── UsageHistory.vue
│   ├── pending/
│   │   ├── PendingPanel.vue
│   │   ├── PendingCard.vue
│   │   └── MarkdownView.vue
│   └── settings/
│       ├── SettingsPanel.vue
│       ├── ProvidersSettings.vue
│       └── AlertsSettings.vue
├── stores/
│   ├── usage.ts
│   └── pending.ts
├── composables/
│   ├── useTauriEvents.ts
│   └── useTerminal.ts
└── types/
    └── index.ts          # TypeScript mirror of Rust structs
```

### 10.2 Tauri communication

**Rust → Vue events:**

| Event | Payload | Description |
|-----------|---------|-------------|
| `usage:updated` | `SessionSummary[]` | Usage update |
| `usage:alert` | `QuotaAlert` | Threshold exceeded |
| `pending:detected` | `PendingPrompt` | New pending prompt |
| `pending:resolved` | `{ pane_id: string }` | Prompt resolved |
| `panes:changed` | `PaneInfo[]` | Open panes change |

**Vue → Rust commands:**

| Command | Parameters | Returns |
|----------|-----------|--------|
| `list_panes` | — | `PaneInfo[]` |
| `focus_pane` | `pane_id: string` | `void` |
| `send_to_pane` | `pane_id, text` | `void` |
| `get_usage_summary` | — | `SessionSummary[]` |
| `get_usage_history` | `days: number` | `UsageEvent[]` |
| `get_pending_prompts` | — | `PendingPrompt[]` |
| `update_settings` | `Settings` | `void` |

---

## 11. Configuration

The configuration file is located via `PathResolver::config_dir()`:

```
Windows  → C:\Users\<user>\AppData\Roaming\ai-shepherd\config.toml
macOS    → ~/Library/Application Support/ai-shepherd/config.toml
Linux    → ~/.config/ai-shepherd/config.toml
```

Content:

```toml
[general]
poll_interval_ms = 2000
theme = "auto"          # "auto" | "light" | "dark"
start_minimized = true

[terminal]
adapter = "wezterm"     # Key in the TerminalRegistry

[providers.claude-code]
enabled = true
token_limit = 45000
alert_thresholds = [75, 90, 100]
# Additional paths ([] = auto-detection via PathResolver)
extra_paths = []

[providers.codex]
enabled = true
alert_thresholds = [80, 100]

[providers.gemini]
enabled = false

[pending]
enabled = true
show_notifications = true
min_urgency = "low"     # "low" | "medium" | "high"

[export]
# Status file path (compatible with Claude Code statusLine)
# Default value resolved via PathResolver::data_dir()
status_file = ""        # "" = default path
```

---

## 12. Rust crate dependencies

| Crate | Usage | Notes |
|-------|-------|-------|
| `tauri` v2 | Desktop framework | Cross-platform |
| `tauri-plugin-notification` | Notifications | Cross-platform |
| `serde` + `serde_json` | Serialization | — |
| `tokio` | Async runtime | — |
| `notify` | File watcher | Cross-platform |
| `rusqlite` | SQLite | Cross-platform |
| `strip-ansi-escapes` | Terminal cleanup | — |
| `chrono` | Dates | — |
| `which` | PATH binary detection | Cross-platform |
| `dirs` | Standard OS paths | **Cross-platform key** |
| `tracing` | Structured logging | — |
| `anyhow` | Error handling | — |

The `dirs` crate is a standard helper for OS directory conventions. It is useful, but not a full platform abstraction layer.

---

## 13. npm dependencies

| Package | Usage |
|---------|-------|
| `vue` v3 | UI framework |
| `pinia` | State management |
| `@tauri-apps/api` v2 | Tauri bindings |
| `markdown-it` | Markdown rendering |
| `markdown-it-highlight` | Syntax highlighting |
| `date-fns` | Date formatting |
| `chart.js` | Usage charts |

---

## 14. Compatibility matrix

| Feature | Windows | macOS | Linux |
|----------------|---------|-------|-------|
| WezTermAdapter | ✅ | ✅ | ✅ |
| Usage Monitor (native files) | ✅ | ✅ | ✅ |
| Usage Monitor (WSL2 files) | ✅ | ➖ n/a | ➖ n/a |
| Pending Prompts | ✅ | ✅ | ✅ |
| System tray | ✅ | ✅ | ⚠️ DE-dependent |
| Notifications | ✅ | ✅ | ✅ |
| SQLite history | ✅ | ✅ | ✅ |
| Path auto-detection | ✅ | ✅ | ✅ |
| WSL2 paths (UNC) | ✅ | ➖ n/a | ➖ n/a |

**Legend:** ✅ supported · ⚠️ supported with condition · ➖ not applicable

> **Linux tray note**: Vanilla GNOME without an extension may not expose a tray. The app must detect that case at startup, show a visible main window, and keep tray actions available where the desktop supports StatusNotifierItem/libappindicator. KDE Plasma, XFCE, i3, and Sway are expected to work, but this is still environment-dependent.

---

## 15. Suggested implementation roadmap

### Phase 1 — Foundations (week 1-2)
- [ ] Scaffold Tauri 2 + Vue 3 + TypeScript
- [ ] `platform/` module: `PathResolver` with `dirs`
- [ ] `TerminalAdapter` trait + `WezTermAdapter` (list_panes, send_text, capture_pane)
- [ ] Basic Tauri commands (`list_panes`, `focus_pane`)
- [ ] UI skeleton: tray, main window, 2-panel navigation

### Phase 2 — Usage Monitor (week 3-4)
- [ ] `UsageFileAdapter` trait + `ClaudeCodeAdapter` (parse JSONL)
- [ ] `FileWatcher` with `notify` for native Windows paths; polling/helper strategy for WSL UNC paths
- [ ] `UsageStore` + SQLite persistence via `PathResolver::data_dir()`
- [ ] Vue `ProviderCard` with progress bar
- [ ] Alerts + notifications

### Phase 3 — Pending Prompts (week 5-6)
- [ ] `ProviderAdapter` trait + `ClaudeCodeProvider`
- [ ] `PendingPoller` background thread
- [ ] Terminal → Markdown conversion
- [ ] `PendingPanel` + `PendingCard` + `MarkdownView`
- [ ] "Focus pane" button

### Phase 4 — Additional providers (week 7)
- [ ] `CodexProvider` + `CodexUsageAdapter`
- [ ] `GeminiProvider` + `GeminiUsageAdapter`
- [ ] Full settings
- [ ] `status.json` export

### Phase 5 — Cross-platform & extensibility (future)
- [ ] macOS tests (CI GitHub Actions macOS runner)
- [ ] Linux tests (CI Ubuntu)
- [ ] Linux tray fallback (GNOME detection)
- [ ] `WindowsTerminalAdapter` (via `wt.exe` CLI)
- [ ] `KittyAdapter` (macOS/Linux)
- [ ] Auto-update via Tauri updater

---

## 16. Security considerations

- Captured prompts may contain sensitive information. They never leave the local machine.
- Do not persist raw prompts by default; if persistence is added, make it opt-in and redact-by-default.
- `send_text` validates the `pane_id` before sending.
- No network communication outside the app's scope.
- Unencrypted SQLite in v1 (planned for v2).
- On Windows, WSL2 UNC paths (`\\wsl$\...`) are accessible without elevated privileges.

---

*Spec generated for implementation — version 0.2-draft*
