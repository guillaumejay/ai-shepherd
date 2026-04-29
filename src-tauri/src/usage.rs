use crate::platform::PathResolver;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    convert::TryFrom,
    fs::File,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AccountType {
    Unknown,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UsagePeriod {
    Current,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WatchDescriptor {
    pub path: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UsageEvent {
    pub event_key: String,
    pub provider_id: String,
    pub source_file: String,
    pub timestamp: Option<DateTime<Utc>>,
    pub session_id: Option<String>,
    pub session_title: Option<String>,
    pub model: Option<String>,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub requests_used: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionSummary {
    pub provider_id: String,
    pub account_type: AccountType,
    pub period: UsagePeriod,
    pub session_id: Option<String>,
    pub session_title: Option<String>,
    pub tokens_used: u64,
    pub requests_used: u64,
    pub cost: f64,
    pub token_limit: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UsageDiagnostics {
    pub provider_id: String,
    pub candidate_files: usize,
    pub parsed_files: usize,
    pub token_events: usize,
    pub summaries: usize,
}

pub trait UsageFileAdapter {
    fn provider_id(&self) -> &'static str;
    fn watch_descriptors(&self) -> Vec<WatchDescriptor>;
    fn recent_session_files(&self) -> Vec<PathBuf>;
    fn parse_file(&self, path: &Path) -> Result<Vec<UsageEvent>, String>;
    fn current_session_summary(&self) -> Result<Vec<SessionSummary>, String>;
}

pub struct ClaudeCodeUsageAdapter;
pub struct CodexUsageAdapter;

struct UsageStore;

struct UsageGroup<'a> {
    session_id: Option<String>,
    session_title: Option<String>,
    events: Vec<&'a UsageEvent>,
    latest: Option<DateTime<Utc>>,
}

impl ClaudeCodeUsageAdapter {
    const RECENT_FILE_LIMIT: usize = 50;

    pub fn new() -> Self {
        Self
    }

    fn candidate_files(&self) -> Vec<PathBuf> {
        let mut files = Vec::new();
        for base in PathResolver::all_candidates(".claude/projects") {
            collect_jsonl_files(&base, &mut files);
        }
        for history_file in PathResolver::all_candidates(".claude/history.jsonl") {
            if history_file.is_file() {
                files.push(history_file);
            }
        }
        files
    }

    fn recent_session_files(&self) -> Vec<PathBuf> {
        let mut files: Vec<_> = self
            .candidate_files()
            .into_iter()
            .filter(|path| {
                !path
                    .components()
                    .any(|component| component.as_os_str() == "subagents")
            })
            .filter_map(|path| {
                let modified = std::fs::metadata(&path).ok()?.modified().ok()?;
                Some((modified, path))
            })
            .collect();

        files.sort_by(|a, b| b.0.cmp(&a.0));
        files
            .into_iter()
            .take(Self::RECENT_FILE_LIMIT)
            .map(|(_, path)| path)
            .collect()
    }

    pub fn all_events(&self) -> Result<Vec<UsageEvent>, String> {
        all_events_from_files(self, self.candidate_files())
    }

    pub fn diagnostics(&self) -> UsageDiagnostics {
        diagnostics_from_adapter(self)
    }
}

impl CodexUsageAdapter {
    const RECENT_FILE_LIMIT: usize = 50;

    pub fn new() -> Self {
        Self
    }

    fn candidate_files(&self) -> Vec<PathBuf> {
        let mut files = Vec::new();
        for base in PathResolver::all_candidates(".codex/sessions") {
            collect_jsonl_files(&base, &mut files);
        }
        files
    }

    fn recent_session_files(&self) -> Vec<PathBuf> {
        recent_files(self.candidate_files(), Self::RECENT_FILE_LIMIT)
    }

    pub fn all_events(&self) -> Result<Vec<UsageEvent>, String> {
        all_events_from_adapter(self)
    }

    pub fn diagnostics(&self) -> UsageDiagnostics {
        diagnostics_from_adapter(self)
    }
}

impl UsageStore {
    fn db_path() -> PathBuf {
        PathResolver::data_dir().join("usage.db")
    }

    fn open() -> Result<Connection, String> {
        std::fs::create_dir_all(PathResolver::data_dir()).map_err(|e| e.to_string())?;
        let conn = Connection::open_with_flags(
            Self::db_path(),
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
        )
        .map_err(|e| e.to_string())?;
        conn.busy_timeout(std::time::Duration::from_secs(5))
            .map_err(|e| e.to_string())?;
        conn.pragma_update(None, "journal_mode", "WAL")
            .map_err(|e| e.to_string())?;
        Ok(conn)
    }

    fn ensure_schema(conn: &Connection) -> Result<(), String> {
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS usage_events (
                event_key TEXT PRIMARY KEY,
                provider_id TEXT NOT NULL,
                source_file TEXT NOT NULL,
                timestamp TEXT,
                session_id TEXT,
                session_title TEXT,
                model TEXT,
                input_tokens INTEGER NOT NULL,
                output_tokens INTEGER NOT NULL,
                requests_used INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_usage_events_timestamp ON usage_events(timestamp);
            CREATE INDEX IF NOT EXISTS idx_usage_events_provider ON usage_events(provider_id);
            "#,
        )
        .map_err(|e| e.to_string())
    }

    fn load_events(conn: &Connection, limit: usize) -> Result<Vec<UsageEvent>, String> {
        let mut stmt = conn
            .prepare(
                r#"
                SELECT event_key, provider_id, source_file, timestamp, session_id, session_title, model,
                       input_tokens, output_tokens, requests_used
                FROM usage_events
                ORDER BY timestamp IS NULL, timestamp DESC, rowid DESC
                LIMIT ?1
                "#,
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![limit as i64], |row| {
                let timestamp: Option<String> = row.get(3)?;
                Ok(UsageEvent {
                    event_key: row.get(0)?,
                    provider_id: row.get(1)?,
                    source_file: row.get(2)?,
                    timestamp: timestamp
                        .as_deref()
                        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                        .map(|dt| dt.with_timezone(&Utc)),
                    session_id: row.get(4)?,
                    session_title: row.get(5)?,
                    model: row.get(6)?,
                    input_tokens: row.get::<_, i64>(7)? as u64,
                    output_tokens: row.get::<_, i64>(8)? as u64,
                    requests_used: row.get::<_, i64>(9)? as u64,
                })
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }

    fn stored_summaries(conn: &Connection) -> Result<Vec<SessionSummary>, String> {
        let events = Self::load_events(conn, 1000)?;
        if events.is_empty() {
            return current_usage_summaries();
        }
        Ok(summaries_from_events_by_provider(&events))
    }
}

fn ingestion_guard() -> &'static Mutex<()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD.get_or_init(|| Mutex::new(()))
}

impl Default for ClaudeCodeUsageAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for CodexUsageAdapter {
    fn default() -> Self {
        Self::new()
    }
}

pub fn current_usage_summaries() -> Result<Vec<SessionSummary>, String> {
    let mut summaries = Vec::new();
    summaries.extend(ClaudeCodeUsageAdapter::new().current_session_summary()?);
    summaries.extend(CodexUsageAdapter::new().current_session_summary()?);
    Ok(summaries)
}

pub fn current_usage_history() -> Result<Vec<UsageEvent>, String> {
    let mut events = Vec::new();
    events.extend(ClaudeCodeUsageAdapter::new().all_events()?);
    events.extend(CodexUsageAdapter::new().all_events()?);
    events.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    events.truncate(1000);
    Ok(events)
}

pub fn ingest_recent_usage_events() -> Result<usize, String> {
    let _lock = ingestion_guard().lock().map_err(|e| e.to_string())?;
    let claude = ClaudeCodeUsageAdapter::new();
    let codex = CodexUsageAdapter::new();
    let conn = UsageStore::open()?;
    UsageStore::ensure_schema(&conn)?;

    let mut conn = conn;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let mut stmt = tx
        .prepare_cached(
            r#"
            INSERT INTO usage_events (
                event_key, provider_id, source_file, timestamp, session_id,
                session_title, model, input_tokens, output_tokens, requests_used
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            ON CONFLICT(event_key) DO UPDATE SET
                provider_id = excluded.provider_id,
                source_file = excluded.source_file,
                timestamp = excluded.timestamp,
                session_id = excluded.session_id,
                session_title = excluded.session_title,
                model = excluded.model,
                input_tokens = excluded.input_tokens,
                output_tokens = excluded.output_tokens,
                requests_used = excluded.requests_used
            "#,
        )
        .map_err(|e| e.to_string())?;

    let mut affected = 0usize;
    for adapter in [
        &claude as &dyn UsageFileAdapter,
        &codex as &dyn UsageFileAdapter,
    ] {
        for file in adapter.recent_session_files() {
            let parsed = match adapter.parse_file(&file) {
                Ok(events) => events,
                Err(_) => continue,
            };
            for event in parsed {
                affected += stmt
                    .execute(params![
                        event.event_key,
                        event.provider_id,
                        event.source_file,
                        event.timestamp.map(|ts| ts.to_rfc3339()),
                        event.session_id,
                        event.session_title,
                        event.model,
                        event.input_tokens,
                        event.output_tokens,
                        event.requests_used,
                    ])
                    .map_err(|e| e.to_string())?;
            }
        }
    }
    drop(stmt);
    tx.commit().map_err(|e| e.to_string())?;
    Ok(affected)
}

pub fn stored_usage_history() -> Result<Vec<UsageEvent>, String> {
    let conn = UsageStore::open()?;
    UsageStore::ensure_schema(&conn)?;
    UsageStore::load_events(&conn, 1000)
}

pub fn stored_usage_summaries() -> Result<Vec<SessionSummary>, String> {
    let conn = UsageStore::open()?;
    UsageStore::ensure_schema(&conn)?;
    UsageStore::stored_summaries(&conn)
}

pub fn usage_diagnostics() -> UsageDiagnostics {
    let claude = ClaudeCodeUsageAdapter::new().diagnostics();
    let codex = CodexUsageAdapter::new().diagnostics();

    UsageDiagnostics {
        provider_id: "all".to_string(),
        candidate_files: claude.candidate_files + codex.candidate_files,
        parsed_files: claude.parsed_files + codex.parsed_files,
        token_events: claude.token_events + codex.token_events,
        summaries: claude.summaries + codex.summaries,
    }
}

impl UsageFileAdapter for ClaudeCodeUsageAdapter {
    fn provider_id(&self) -> &'static str {
        "claude-code"
    }

    fn watch_descriptors(&self) -> Vec<WatchDescriptor> {
        let mut paths = PathResolver::all_candidates(".claude/projects");
        paths.extend(PathResolver::all_candidates(".claude/history.jsonl"));
        paths
            .into_iter()
            .map(|path| WatchDescriptor {
                path: path.to_string_lossy().to_string(),
            })
            .collect()
    }

    fn recent_session_files(&self) -> Vec<PathBuf> {
        ClaudeCodeUsageAdapter::recent_session_files(self)
    }

    fn parse_file(&self, path: &Path) -> Result<Vec<UsageEvent>, String> {
        if path.file_name().and_then(|name| name.to_str()) == Some("history.jsonl") {
            return parse_claude_history_file(path, self.provider_id());
        }

        parse_jsonl_file(path, self.provider_id())
    }

    fn current_session_summary(&self) -> Result<Vec<SessionSummary>, String> {
        let mut events = Vec::new();
        for file in self.recent_session_files() {
            if let Ok(mut parsed) = self.parse_file(&file) {
                events.append(&mut parsed);
            }
        }
        if events.is_empty() {
            return Ok(vec![]);
        }
        Ok(summaries_from_events(&events, self.provider_id()))
    }
}

impl UsageFileAdapter for CodexUsageAdapter {
    fn provider_id(&self) -> &'static str {
        "codex"
    }

    fn watch_descriptors(&self) -> Vec<WatchDescriptor> {
        PathResolver::all_candidates(".codex/sessions")
            .into_iter()
            .map(|path| WatchDescriptor {
                path: path.to_string_lossy().to_string(),
            })
            .collect()
    }

    fn recent_session_files(&self) -> Vec<PathBuf> {
        CodexUsageAdapter::recent_session_files(self)
    }

    fn parse_file(&self, path: &Path) -> Result<Vec<UsageEvent>, String> {
        parse_codex_jsonl_file(path, self.provider_id())
    }

    fn current_session_summary(&self) -> Result<Vec<SessionSummary>, String> {
        let mut events = Vec::new();
        for file in self.recent_session_files() {
            if let Ok(mut parsed) = self.parse_file(&file) {
                events.append(&mut parsed);
            }
        }
        if events.is_empty() {
            return Ok(vec![]);
        }
        Ok(summaries_from_events(&events, self.provider_id()))
    }
}

fn summaries_from_events(events: &[UsageEvent], provider_id: &str) -> Vec<SessionSummary> {
    let summary_events: Vec<_> = events.iter().collect();

    if summary_events.is_empty() {
        return vec![];
    }

    let now = Utc::now();
    let window_start = now - chrono::Duration::hours(5);
    let timestamped_in_window = summary_events
        .iter()
        .any(|event| event.timestamp.is_some_and(|ts| ts >= window_start));

    let selected_events: Vec<&UsageEvent> = if timestamped_in_window {
        summary_events
            .into_iter()
            .filter(|event| event.timestamp.is_some_and(|ts| ts >= window_start))
            .collect()
    } else {
        let latest_date = summary_events
            .iter()
            .filter_map(|event| event.timestamp.map(|ts| ts.date_naive()))
            .max();

        match latest_date {
            Some(date) => summary_events
                .into_iter()
                .filter(|event| event.timestamp.is_some_and(|ts| ts.date_naive() == date))
                .collect(),
            None => summary_events.into_iter().collect(),
        }
    };

    if selected_events.is_empty() {
        return vec![];
    }

    let mut grouped: HashMap<String, UsageGroup<'_>> = HashMap::new();
    for event in selected_events {
        let (group_id, session_id) = match event.session_id.as_ref() {
            Some(session_id) => (format!("session:{session_id}"), Some(session_id.clone())),
            None => (format!("file:{}", event.source_file), None),
        };

        let entry = grouped.entry(group_id).or_insert_with(|| UsageGroup {
            session_id,
            session_title: event.session_title.clone(),
            events: Vec::new(),
            latest: event.timestamp,
        });

        if entry.session_title.is_none() {
            entry.session_title = event.session_title.clone();
        }
        entry.events.push(event);
        if event.timestamp > entry.latest {
            entry.latest = event.timestamp;
        }
    }

    let mut grouped: Vec<_> = grouped.into_values().collect();
    grouped.sort_by(|a, b| b.latest.cmp(&a.latest));

    grouped
        .into_iter()
        .map(|group| SessionSummary {
            provider_id: provider_id.to_string(),
            account_type: AccountType::Unknown,
            period: UsagePeriod::Current,
            session_id: group.session_id,
            session_title: group.session_title,
            tokens_used: group
                .events
                .iter()
                .map(|event| event.input_tokens + event.output_tokens)
                .sum(),
            requests_used: group.events.iter().map(|event| event.requests_used).sum(),
            cost: 0.0,
            token_limit: None,
        })
        .collect()
}

fn summaries_from_events_by_provider(events: &[UsageEvent]) -> Vec<SessionSummary> {
    let mut provider_ids: Vec<_> = events
        .iter()
        .map(|event| event.provider_id.as_str())
        .collect();
    provider_ids.sort_unstable();
    provider_ids.dedup();

    let mut summaries = Vec::new();
    for provider_id in provider_ids {
        let provider_events: Vec<_> = events
            .iter()
            .filter(|event| event.provider_id == provider_id)
            .cloned()
            .collect();
        summaries.extend(summaries_from_events(&provider_events, provider_id));
    }
    summaries.sort_by(|a, b| b.tokens_used.cmp(&a.tokens_used));
    summaries
}

fn recent_files(files: Vec<PathBuf>, limit: usize) -> Vec<PathBuf> {
    let mut files: Vec<_> = files
        .into_iter()
        .filter_map(|path| {
            let modified = std::fs::metadata(&path).ok()?.modified().ok()?;
            Some((modified, path))
        })
        .collect();

    files.sort_by(|a, b| b.0.cmp(&a.0));
    files
        .into_iter()
        .take(limit)
        .map(|(_, path)| path)
        .collect()
}

fn all_events_from_adapter(adapter: &impl UsageFileAdapter) -> Result<Vec<UsageEvent>, String> {
    all_events_from_files(adapter, adapter.recent_session_files())
}

fn all_events_from_files(
    adapter: &impl UsageFileAdapter,
    files: Vec<PathBuf>,
) -> Result<Vec<UsageEvent>, String> {
    let mut events = Vec::new();
    for file in files {
        if let Ok(mut parsed) = adapter.parse_file(&file) {
            events.append(&mut parsed);
        }
    }
    Ok(events)
}

fn diagnostics_from_adapter(adapter: &impl UsageFileAdapter) -> UsageDiagnostics {
    let files = adapter.recent_session_files();
    let mut parsed_files = 0usize;
    let mut token_events = 0usize;

    for file in &files {
        if let Ok(events) = adapter.parse_file(file) {
            parsed_files += 1;
            token_events += events.len();
        }
    }

    let summaries = adapter
        .current_session_summary()
        .map_or(0, |items| items.len());

    UsageDiagnostics {
        provider_id: adapter.provider_id().to_string(),
        candidate_files: files.len(),
        parsed_files,
        token_events,
        summaries,
    }
}

fn collect_jsonl_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_jsonl_files(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
            out.push(path);
        }
    }
}

fn parse_jsonl_file(path: &Path, provider_id: &str) -> Result<Vec<UsageEvent>, String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    let reader = BufReader::new(file);
    let mut values: Vec<(usize, serde_json::Value)> = Vec::new();

    for (line_number, line) in reader
        .lines()
        .enumerate()
        .filter_map(|(idx, line)| line.ok().map(|l| (idx + 1, l)))
    {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) else {
            continue;
        };
        values.push((line_number, value));
    }

    let mut session_titles: HashMap<String, String> = HashMap::new();
    let mut custom_title: Option<String> = None;
    let mut ai_title: Option<String> = None;
    let mut first_user_label: Option<String> = None;
    for (_, value) in &values {
        if let Some(title) = extract_explicit_session_title(value) {
            if let Some(session_id) = extract_session_id(value) {
                session_titles.insert(session_id, title.clone());
            }
            match title_kind(value) {
                Some(TitleKind::Custom) => custom_title = Some(title),
                Some(TitleKind::Ai) => ai_title = Some(title),
                None => {}
            }
        } else if first_user_label.is_none() {
            if let Some(user_text) = extract_first_user_text(value) {
                first_user_label = Some(truncate_unicode_scalars(&user_text, 60));
            }
        }
    }
    let file_session_label = custom_title.or(ai_title).or(first_user_label);

    let mut events = Vec::new();
    for (line_number, value) in values.into_iter() {
        let input_tokens = nested_u64(&value, &["usage", "input_tokens"])
            .or_else(|| nested_u64(&value, &["message", "usage", "input_tokens"]))
            .unwrap_or(0);
        let output_tokens = nested_u64(&value, &["usage", "output_tokens"])
            .or_else(|| nested_u64(&value, &["message", "usage", "output_tokens"]))
            .unwrap_or(0);
        if input_tokens + output_tokens == 0 {
            continue;
        }
        let timestamp = nested_string(&value, &["timestamp"])
            .or_else(|| nested_string(&value, &["created_at"]))
            .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
            .map(|dt| dt.with_timezone(&Utc));
        let session_id = extract_session_id(&value);
        let session_title = extract_explicit_session_title(&value)
            .or_else(|| {
                session_id
                    .as_ref()
                    .and_then(|id| session_titles.get(id).cloned())
            })
            .or_else(|| file_session_label.clone());
        let model = nested_string(&value, &["model"])
            .or_else(|| nested_string(&value, &["message", "model"]));

        events.push(UsageEvent {
            event_key: format!("{}|{}|{}", provider_id, path.to_string_lossy(), line_number),
            provider_id: provider_id.to_string(),
            source_file: path.to_string_lossy().to_string(),
            timestamp,
            session_id,
            session_title,
            model,
            input_tokens,
            output_tokens,
            requests_used: 1,
        });
    }

    Ok(events)
}

fn parse_claude_history_file(path: &Path, provider_id: &str) -> Result<Vec<UsageEvent>, String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    let reader = BufReader::new(file);
    let mut events = Vec::new();

    for (line_number, line) in reader
        .lines()
        .enumerate()
        .filter_map(|(idx, line)| line.ok().map(|l| (idx + 1, l)))
    {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) else {
            continue;
        };
        let Some(session_id) = nested_string(&value, &["sessionId"]) else {
            continue;
        };
        let timestamp = nested_u64(&value, &["timestamp"])
            .and_then(|millis| i64::try_from(millis).ok())
            .and_then(DateTime::<Utc>::from_timestamp_millis);
        let session_title = nested_string(&value, &["display"])
            .or_else(|| nested_string(&value, &["project"]))
            .map(|title| truncate_unicode_scalars(&title, 60));

        events.push(UsageEvent {
            event_key: format!("{}|{}|{}", provider_id, path.to_string_lossy(), line_number),
            provider_id: provider_id.to_string(),
            source_file: path.to_string_lossy().to_string(),
            timestamp,
            session_id: Some(session_id),
            session_title,
            model: None,
            input_tokens: 0,
            output_tokens: 0,
            requests_used: 0,
        });
    }

    Ok(events)
}

fn parse_codex_jsonl_file(path: &Path, provider_id: &str) -> Result<Vec<UsageEvent>, String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    let reader = BufReader::new(file);
    let mut values: Vec<(usize, serde_json::Value)> = Vec::new();

    for (line_number, line) in reader
        .lines()
        .enumerate()
        .filter_map(|(idx, line)| line.ok().map(|l| (idx + 1, l)))
    {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) else {
            continue;
        };
        values.push((line_number, value));
    }

    let mut session_id = None;
    let mut session_title = None;
    let mut model = None;
    for (_, value) in &values {
        if session_id.is_none() {
            session_id = nested_string(value, &["payload", "id"])
                .filter(|_| value.get("type").and_then(|v| v.as_str()) == Some("session_meta"))
                .or_else(|| nested_string(value, &["payload", "session_id"]))
                .or_else(|| nested_string(value, &["session_id"]));
        }
        if session_title.is_none() {
            session_title =
                extract_codex_user_text(value).map(|text| truncate_unicode_scalars(&text, 60));
        }
        if let Some(turn_model) = nested_string(value, &["payload", "model"]) {
            model = Some(turn_model);
        } else if model.is_none() {
            model = nested_string(value, &["payload", "model_provider"]);
        }
    }
    let session_id = session_id.or_else(|| codex_session_id_from_path(path));

    let mut events = Vec::new();
    let mut previous_input = 0u64;
    let mut previous_output = 0u64;

    for (line_number, value) in values.into_iter() {
        if value.get("type").and_then(|v| v.as_str()) != Some("event_msg")
            || nested_string(&value, &["payload", "type"]).as_deref() != Some("token_count")
        {
            continue;
        }

        let Some(total_input) = nested_u64(
            &value,
            &["payload", "info", "total_token_usage", "input_tokens"],
        ) else {
            continue;
        };
        let Some(total_output) = nested_u64(
            &value,
            &["payload", "info", "total_token_usage", "output_tokens"],
        ) else {
            continue;
        };

        let input_tokens = total_input.saturating_sub(previous_input);
        let output_tokens = total_output.saturating_sub(previous_output);
        previous_input = total_input;
        previous_output = total_output;

        if input_tokens + output_tokens == 0 {
            continue;
        }

        let timestamp = nested_string(&value, &["timestamp"])
            .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
            .map(|dt| dt.with_timezone(&Utc));

        events.push(UsageEvent {
            event_key: format!("{}|{}|{}", provider_id, path.to_string_lossy(), line_number),
            provider_id: provider_id.to_string(),
            source_file: path.to_string_lossy().to_string(),
            timestamp,
            session_id: session_id.clone(),
            session_title: session_title.clone(),
            model: model.clone(),
            input_tokens,
            output_tokens,
            requests_used: 1,
        });
    }

    Ok(events)
}

fn codex_session_id_from_path(path: &Path) -> Option<String> {
    let stem = path.file_stem()?.to_str()?;
    if let Some(value) = stem.strip_prefix("rollout-") {
        if value.len() >= 36 {
            return Some(value[value.len() - 36..].to_string());
        }
    }

    Some(stem.to_string())
}

fn extract_codex_user_text(value: &serde_json::Value) -> Option<String> {
    if value.get("type").and_then(|v| v.as_str()) == Some("event_msg")
        && nested_string(value, &["payload", "type"]).as_deref() == Some("user_message")
    {
        return nested_string(value, &["payload", "message"]);
    }

    if value.get("type").and_then(|v| v.as_str()) == Some("response_item")
        && nested_string(value, &["payload", "type"]).as_deref() == Some("message")
        && nested_string(value, &["payload", "role"]).as_deref() == Some("user")
    {
        return content_text(value.get("payload").and_then(|p| p.get("content")));
    }

    None
}

fn nested_u64(value: &serde_json::Value, path: &[&str]) -> Option<u64> {
    nested_value(value, path)?.as_u64().or_else(|| {
        nested_value(value, path)?
            .as_i64()
            .and_then(|v| u64::try_from(v).ok())
    })
}

fn nested_string(value: &serde_json::Value, path: &[&str]) -> Option<String> {
    nested_value(value, path)?
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

fn extract_session_id(value: &serde_json::Value) -> Option<String> {
    nested_string(value, &["session_id"])
        .or_else(|| nested_string(value, &["sessionId"]))
        .or_else(|| nested_string(value, &["session", "id"]))
}

fn extract_explicit_session_title(value: &serde_json::Value) -> Option<String> {
    title_from_entry(value)
}

fn title_from_entry(value: &serde_json::Value) -> Option<String> {
    match value.get("type")?.as_str()? {
        "custom-title" => nested_string(value, &["name"]),
        "ai-title" => nested_string(value, &["title"]),
        _ => None,
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum TitleKind {
    Custom,
    Ai,
}

fn title_kind(value: &serde_json::Value) -> Option<TitleKind> {
    match value.get("type")?.as_str()? {
        "custom-title" => Some(TitleKind::Custom),
        "ai-title" => Some(TitleKind::Ai),
        _ => None,
    }
}

fn extract_first_user_text(value: &serde_json::Value) -> Option<String> {
    if is_user_entry(value) {
        if let Some(text) = content_text(value.get("message").and_then(|m| m.get("content"))) {
            return Some(text);
        }
        if let Some(text) = content_text(value.get("content")) {
            return Some(text);
        }
    }

    if value
        .get("message")
        .and_then(|m| m.get("role"))
        .and_then(|r| r.as_str())
        == Some("user")
    {
        if let Some(text) = content_text(value.get("message").and_then(|m| m.get("content"))) {
            return Some(text);
        }
    }

    None
}

fn is_user_entry(value: &serde_json::Value) -> bool {
    value.get("type").and_then(|v| v.as_str()) == Some("user")
        || value
            .get("message")
            .and_then(|m| m.get("role"))
            .and_then(|r| r.as_str())
            == Some("user")
}

fn content_text(value: Option<&serde_json::Value>) -> Option<String> {
    let value = value?;
    if let Some(text) = value.as_str().map(str::trim).filter(|s| !s.is_empty()) {
        return Some(text.to_string());
    }
    let mut parts = Vec::new();
    if let Some(items) = value.as_array() {
        for item in items {
            if let Some(text) = item
                .get("text")
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                parts.push(text.to_string());
            }
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(""))
    }
}

fn truncate_unicode_scalars(input: &str, max_chars: usize) -> String {
    let mut out = String::new();
    for (idx, ch) in input.chars().enumerate() {
        if idx >= max_chars {
            out.push('…');
            return out;
        }
        out.push(ch);
    }
    out
}

fn nested_value<'a>(value: &'a serde_json::Value, path: &[&str]) -> Option<&'a serde_json::Value> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    Some(current)
}
