use crate::platform::PathResolver;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    convert::TryFrom,
    fs::File,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
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
    fn parse_file(&self, path: &Path) -> Result<Vec<UsageEvent>, String>;
    fn current_session_summary(&self) -> Result<Vec<SessionSummary>, String>;
}

pub struct ClaudeCodeUsageAdapter;

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
        let mut events = Vec::new();
        for file in self.candidate_files() {
            if let Ok(mut parsed) = self.parse_file(&file) {
                events.append(&mut parsed);
            }
        }
        Ok(events)
    }

    pub fn diagnostics(&self) -> UsageDiagnostics {
        let files = self.recent_session_files();
        let mut parsed_files = 0usize;
        let mut token_events = 0usize;

        for file in &files {
            if let Ok(events) = self.parse_file(file) {
                parsed_files += 1;
                token_events += events.len();
            }
        }

        let summaries = self
            .current_session_summary()
            .map_or(0, |items| items.len());

        UsageDiagnostics {
            provider_id: self.provider_id().to_string(),
            candidate_files: files.len(),
            parsed_files,
            token_events,
            summaries,
        }
    }
}

impl Default for ClaudeCodeUsageAdapter {
    fn default() -> Self {
        Self::new()
    }
}

pub fn current_usage_summaries() -> Result<Vec<SessionSummary>, String> {
    ClaudeCodeUsageAdapter::new().current_session_summary()
}

pub fn usage_diagnostics() -> UsageDiagnostics {
    ClaudeCodeUsageAdapter::new().diagnostics()
}

impl UsageFileAdapter for ClaudeCodeUsageAdapter {
    fn provider_id(&self) -> &'static str {
        "claude-code"
    }

    fn watch_descriptors(&self) -> Vec<WatchDescriptor> {
        PathResolver::all_candidates(".claude/projects")
            .into_iter()
            .map(|path| WatchDescriptor {
                path: path.to_string_lossy().to_string(),
            })
            .collect()
    }

    fn parse_file(&self, path: &Path) -> Result<Vec<UsageEvent>, String> {
        parse_jsonl_file(path, self.provider_id())
    }

    fn current_session_summary(&self) -> Result<Vec<SessionSummary>, String> {
        let mut events = Vec::new();
        for file in self.recent_session_files() {
            if let Ok(mut parsed) = self.parse_file(&file) {
                events.append(&mut parsed);
            }
        }
        let token_events: Vec<_> = events
            .into_iter()
            .filter(|event| event.input_tokens + event.output_tokens > 0)
            .collect();

        if token_events.is_empty() {
            return Ok(vec![]);
        }

        let now = Utc::now();
        let window_start = now - chrono::Duration::hours(5);
        let timestamped_in_window = token_events
            .iter()
            .any(|event| event.timestamp.is_some_and(|ts| ts >= window_start));

        let selected_events: Vec<&UsageEvent> = if timestamped_in_window {
            token_events
                .iter()
                .filter(|event| event.timestamp.is_some_and(|ts| ts >= window_start))
                .collect()
        } else {
            let latest_date = token_events
                .iter()
                .filter_map(|event| event.timestamp.map(|ts| ts.date_naive()))
                .max();

            match latest_date {
                Some(date) => token_events
                    .iter()
                    .filter(|event| event.timestamp.is_some_and(|ts| ts.date_naive() == date))
                    .collect(),
                None => token_events.iter().collect(),
            }
        };

        if selected_events.is_empty() {
            return Ok(vec![]);
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

        Ok(grouped
            .into_iter()
            .map(|group| SessionSummary {
                provider_id: self.provider_id().to_string(),
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
            .collect())
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
    let mut values = Vec::new();

    for line in reader.lines().map_while(Result::ok) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) else {
            continue;
        };
        values.push(value);
    }

    let mut session_titles: HashMap<String, String> = HashMap::new();
    let mut custom_title: Option<String> = None;
    let mut ai_title: Option<String> = None;
    let mut first_user_label: Option<String> = None;
    for value in &values {
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
    for value in values {
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
