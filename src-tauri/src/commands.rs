use crate::terminal::{PaneInfo, TerminalAdapter, WezTermAdapter};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UsageSummary {
    pub items: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UsageEvent {
    pub value: String,
}
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
pub fn get_usage_summary() -> Result<UsageSummary, String> {
    Ok(UsageSummary { items: vec![] })
}
#[tauri::command]
pub fn get_usage_history() -> Result<Vec<UsageEvent>, String> {
    Ok(vec![])
}
#[tauri::command]
pub fn get_pending_prompts() -> Result<Vec<PendingPrompt>, String> {
    Ok(vec![])
}
#[tauri::command]
pub fn update_settings(_settings: String) -> Result<(), String> {
    Ok(())
}
