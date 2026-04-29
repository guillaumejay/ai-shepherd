use crate::{
    terminal::{PaneInfo, TerminalAdapter, WezTermAdapter},
    usage::{
        current_usage_history, current_usage_summaries, stored_usage_history,
        stored_usage_summaries, SessionSummary, UsageDiagnostics, UsageEvent,
    },
};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PendingPrompt {
    pub value: String,
}

#[tauri::command]
pub fn list_panes() -> Result<Vec<PaneInfo>, String> {
    let adapter = WezTermAdapter::new();
    adapter.list_panes().map_err(|e| e.to_string())
}
#[tauri::command]
pub fn focus_pane(pane_id: String) -> Result<(), String> {
    WezTermAdapter::new()
        .focus_pane(&pane_id)
        .map_err(|e| e.to_string())
}
#[tauri::command]
pub fn send_to_pane(pane_id: String, text: String) -> Result<(), String> {
    WezTermAdapter::new()
        .send_text(&pane_id, &text)
        .map_err(|e| e.to_string())
}
#[tauri::command]
pub fn get_usage_summary() -> Result<Vec<SessionSummary>, String> {
    stored_usage_summaries().or_else(|_| current_usage_summaries())
}
#[tauri::command]
pub fn get_usage_history() -> Result<Vec<UsageEvent>, String> {
    stored_usage_history().or_else(|_| current_usage_history())
}

#[tauri::command]
pub fn get_usage_diagnostics() -> UsageDiagnostics {
    crate::usage::usage_diagnostics()
}

#[tauri::command]
pub fn get_pending_prompts() -> Result<Vec<PendingPrompt>, String> {
    Ok(vec![])
}

#[tauri::command]
pub fn update_settings(_settings: String) -> Result<(), String> {
    Ok(())
}
