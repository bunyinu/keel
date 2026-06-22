use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::Path;

use crate::paths::{
    append_jsonl, ensure_keel_dir, keel_dir, read_json, utcnow, write_json_atomic, CHANGELOG_FILE,
    CONFIG_FILE, STATE_FILE,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Goal {
    pub title: String,
    #[serde(default)]
    pub acceptance: Vec<String>,
    #[serde(default)]
    pub constraints: Vec<String>,
    pub started_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct Progress {
    pub current_step: Option<String>,
    #[serde(default)]
    pub completed: Vec<String>,
    #[serde(default)]
    pub blockers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Decision {
    pub at: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct KeelState {
    #[serde(default = "default_version")]
    pub version: u32,
    pub goal: Option<Goal>,
    #[serde(default)]
    pub progress: Progress,
    #[serde(default)]
    pub decisions: Vec<Decision>,
    pub compactions: u32,
    pub sessions: u32,
    pub last_agent: Option<String>,
    pub updated_at: Option<String>,
}

impl Default for KeelState {
    fn default() -> Self {
        Self {
            version: default_version(),
            goal: None,
            progress: Progress::default(),
            decisions: vec![],
            compactions: 0,
            sessions: 0,
            last_agent: None,
            updated_at: None,
        }
    }
}

fn default_version() -> u32 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct AcceptanceGateConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub command: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LoopBreakerConfig {
    pub max_same_failure: u32,
    pub window_minutes: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KeelConfig {
    pub loop_breaker: LoopBreakerConfig,
    pub snapshot_max_lines: u32,
    pub snapshot_max_decisions: u32,
    pub snapshot_max_failures: u32,
    #[serde(default)]
    pub acceptance_gate: AcceptanceGateConfig,
    pub installed_at: Option<String>,
}

impl Default for KeelConfig {
    fn default() -> Self {
        Self {
            loop_breaker: LoopBreakerConfig {
                max_same_failure: 2,
                window_minutes: 60,
            },
            snapshot_max_lines: 120,
            snapshot_max_decisions: 8,
            snapshot_max_failures: 6,
            acceptance_gate: AcceptanceGateConfig::default(),
            installed_at: None,
        }
    }
}

pub fn load_state(root: Option<&Path>) -> Result<KeelState> {
    let path = keel_dir(root).join(STATE_FILE);
    let value = read_json(&path, json!(null))?;
    if value.is_null() {
        return Ok(KeelState::default());
    }
    let value = normalize_state_json(value);
    Ok(serde_json::from_value(value)?)
}

/// Fill legacy / cloud-empty state objects so older binaries and `{}` pulls deserialize.
fn normalize_state_json(mut value: serde_json::Value) -> serde_json::Value {
    let Some(obj) = value.as_object_mut() else {
        return value;
    };
    obj.entry("version").or_insert(json!(default_version()));
    obj.entry("progress").or_insert(json!({}));
    obj.entry("decisions").or_insert(json!([]));
    obj.entry("compactions").or_insert(json!(0));
    obj.entry("sessions").or_insert(json!(0));
    value
}

pub fn save_state(state: &mut KeelState, root: Option<&Path>) -> Result<()> {
    state.updated_at = Some(utcnow());
    let path = keel_dir(root).join(STATE_FILE);
    let value = serde_json::to_value(state)?;
    write_json_atomic(&path, &value)
}

pub fn load_config(root: Option<&Path>) -> Result<KeelConfig> {
    let path = keel_dir(root).join(CONFIG_FILE);
    let value = read_json(&path, json!(null))?;
    if value.is_null() {
        return Ok(KeelConfig::default());
    }
    Ok(serde_json::from_value(value)?)
}

pub fn save_config(config: &KeelConfig, root: Option<&Path>) -> Result<()> {
    let path = keel_dir(root).join(CONFIG_FILE);
    let value = serde_json::to_value(config)?;
    write_json_atomic(&path, &value)
}

pub fn log_event(root: Option<&Path>, event: &str, fields: Value) -> Result<()> {
    let mut record = json!({
        "at": utcnow(),
        "event": event,
    });
    if let Some(obj) = record.as_object_mut() {
        if let Some(extra) = fields.as_object() {
            for (k, v) in extra {
                obj.insert(k.clone(), v.clone());
            }
        }
    }
    append_jsonl(&keel_dir(root).join(CHANGELOG_FILE), &record)
}

pub fn log_attempt(
    root: Option<&Path>,
    agent: &str,
    tool: &str,
    action: &str,
    ok: bool,
    detail: &str,
    exit_code: Option<i64>,
) -> Result<()> {
    let mut detail = detail.to_string();
    if detail.len() > 2000 {
        detail.truncate(2000);
    }
    append_jsonl(
        &keel_dir(root).join(crate::paths::ATTEMPTS_FILE),
        &json!({
            "at": utcnow(),
            "agent": agent,
            "tool": tool,
            "action": action,
            "ok": ok,
            "detail": detail,
            "exit_code": exit_code,
        }),
    )
}

pub fn init_config(root: Option<&Path>) -> Result<()> {
    ensure_keel_dir(root)?;
    let mut config = KeelConfig::default();
    config.installed_at = Some(utcnow());
    save_config(&config, root)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join(crate::KEEL_DIR)).unwrap();

        let mut state = KeelState::default();
        state.goal = Some(Goal {
            title: "test".into(),
            acceptance: vec!["a".into()],
            constraints: vec![],
            started_at: utcnow(),
        });
        save_state(&mut state, Some(root)).unwrap();
        let loaded = load_state(Some(root)).unwrap();
        assert_eq!(loaded.goal.unwrap().title, "test");
    }

    #[test]
    fn load_empty_cloud_state() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join(crate::KEEL_DIR)).unwrap();
        std::fs::write(root.join(crate::KEEL_DIR).join(STATE_FILE), "{}\n").unwrap();
        let loaded = load_state(Some(root)).unwrap();
        assert_eq!(loaded.version, default_version());
    }
}
