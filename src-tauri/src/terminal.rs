use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};
use thiserror::Error;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Platform {
    Windows,
    MacOs,
    Linux,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum ShellType {
    PowerShell,
    Bash,
    Zsh,
    Fish,
    Cmd,
    Unknown(String),
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum ShellContext {
    Native,
    Wsl { distro: String },
    Ssh { host: String },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CwdLocation {
    pub raw: String,
    pub normalized: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PaneInfo {
    pub id: String,
    pub terminal_adapter: String,
    pub title: String,
    pub cwd: Option<CwdLocation>,
    pub shell: ShellType,
    pub context: ShellContext,
    pub process_name: Option<String>,
    pub last_seen: DateTime<Utc>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TerminalCapabilities {
    pub list_panes: bool,
    pub capture_pane: bool,
    pub send_text: bool,
    pub focus_pane: bool,
}
#[derive(Error, Debug, Serialize, Deserialize)]
pub enum AdapterError {
    #[error("io error: {0}")]
    Io(String),
    #[error("command failed: {0}")]
    Command(String),
    #[error("json error: {0}")]
    Json(String),
    #[error("command timed out: {0}")]
    Timeout(String),
}
pub trait TerminalAdapter {
    fn id(&self) -> &'static str;
    fn display_name(&self) -> &'static str;
    fn supported_platforms(&self) -> &[Platform];
    fn is_available(&self) -> bool;
    fn list_panes(&self) -> Result<Vec<PaneInfo>, AdapterError>;
    fn capabilities(&self) -> TerminalCapabilities;
    fn send_text(&self, pane_id: &str, text: &str) -> Result<(), AdapterError>;
    fn capture_pane(&self, pane_id: &str) -> Result<String, AdapterError>;
    fn focus_pane(&self, pane_id: &str) -> Result<(), AdapterError>;
}

pub struct WezTermAdapter {
    binary_path: PathBuf,
}

impl WezTermAdapter {
    const COMMAND_TIMEOUT: Duration = Duration::from_secs(5);

    pub fn new() -> Self {
        Self {
            binary_path: which::which("wezterm").unwrap_or_else(|_| PathBuf::from("wezterm")),
        }
    }
    fn run(
        &self,
        args: &[&str],
        stdin: Option<&str>,
    ) -> Result<std::process::Output, AdapterError> {
        let mut cmd = Command::new(&self.binary_path);
        cmd.args(args);
        if stdin.is_some() {
            cmd.stdin(Stdio::piped());
        }
        let mut child = cmd
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| AdapterError::Io(e.to_string()))?;
        if let Some(input) = stdin {
            use std::io::Write;
            child
                .stdin
                .take()
                .ok_or_else(|| AdapterError::Io("missing stdin".into()))?
                .write_all(input.as_bytes())
                .map_err(|e| AdapterError::Io(e.to_string()))?;
        }

        let started = Instant::now();
        while child
            .try_wait()
            .map_err(|e| AdapterError::Io(e.to_string()))?
            .is_none()
        {
            if started.elapsed() >= Self::COMMAND_TIMEOUT {
                let _ = child.kill();
                let _ = child.wait();
                return Err(AdapterError::Timeout(format!("wezterm {}", args.join(" "))));
            }
            thread::sleep(Duration::from_millis(10));
        }

        child
            .wait_with_output()
            .map_err(|e| AdapterError::Io(e.to_string()))
    }
}

impl Default for WezTermAdapter {
    fn default() -> Self {
        Self::new()
    }
}
#[derive(Deserialize)]
struct WezPaneRaw {
    pane_id: serde_json::Value,
    title: Option<String>,
    cwd: Option<String>,
}

impl TerminalAdapter for WezTermAdapter {
    fn id(&self) -> &'static str {
        "wezterm"
    }
    fn display_name(&self) -> &'static str {
        "WezTerm"
    }
    fn supported_platforms(&self) -> &[Platform] {
        &[Platform::Windows, Platform::MacOs, Platform::Linux]
    }
    fn is_available(&self) -> bool {
        which::which("wezterm").is_ok()
    }
    fn list_panes(&self) -> Result<Vec<PaneInfo>, AdapterError> {
        if !self.is_available() {
            return Err(AdapterError::Command(
                "WezTerm executable was not found on PATH".into(),
            ));
        }

        let out = self.run(&["cli", "list", "--format", "json"], None)?;
        if !out.status.success() {
            return Err(AdapterError::Command(
                String::from_utf8_lossy(&out.stderr).into(),
            ));
        }
        let raw: Vec<WezPaneRaw> =
            serde_json::from_slice(&out.stdout).map_err(|e| AdapterError::Json(e.to_string()))?;
        Ok(raw
            .into_iter()
            .map(|p| PaneInfo {
                id: p.pane_id.to_string(),
                terminal_adapter: self.id().into(),
                title: p.title.unwrap_or_default(),
                cwd: p.cwd.map(|raw| CwdLocation {
                    normalized: None,
                    raw,
                }),
                shell: ShellType::Unknown("unknown".into()),
                context: ShellContext::Native,
                process_name: None,
                last_seen: Utc::now(),
            })
            .collect())
    }
    fn capabilities(&self) -> TerminalCapabilities {
        TerminalCapabilities {
            list_panes: true,
            capture_pane: true,
            send_text: true,
            focus_pane: true,
        }
    }
    fn send_text(&self, pane_id: &str, text: &str) -> Result<(), AdapterError> {
        let out = self.run(
            &["cli", "send-text", "--pane-id", pane_id, "--no-paste"],
            Some(text),
        )?;
        if out.status.success() {
            Ok(())
        } else {
            Err(AdapterError::Command(
                String::from_utf8_lossy(&out.stderr).into(),
            ))
        }
    }
    fn capture_pane(&self, pane_id: &str) -> Result<String, AdapterError> {
        let out = self.run(&["cli", "get-text", "--pane-id", pane_id], None)?;
        if !out.status.success() {
            return Err(AdapterError::Command(
                String::from_utf8_lossy(&out.stderr).into(),
            ));
        }
        Ok(String::from_utf8_lossy(&out.stdout).into())
    }
    fn focus_pane(&self, pane_id: &str) -> Result<(), AdapterError> {
        let out = self.run(&["cli", "activate-pane", "--pane-id", pane_id], None)?;
        if out.status.success() {
            Ok(())
        } else {
            Err(AdapterError::Command(
                String::from_utf8_lossy(&out.stderr).into(),
            ))
        }
    }
}
