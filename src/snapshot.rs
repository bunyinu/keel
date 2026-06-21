use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;

use crate::paths::{keel_dir, read_jsonl_tail, SNAPSHOT_FILE, ATTEMPTS_FILE};
use crate::state::{load_config, load_state};

pub fn render_snapshot(root: Option<&Path>) -> Result<String> {
    let state = load_state(root)?;
    let config = load_config(root)?;
    let attempts = read_jsonl_tail(&keel_dir(root).join(ATTEMPTS_FILE), 200)?;

    let mut lines: Vec<String> = vec![
        "# Keel state snapshot".into(),
        String::new(),
        format!(
            "_Compactions: {} · Sessions: {} · Last agent: {}_",
            state.compactions,
            state.sessions,
            state.last_agent.as_deref().unwrap_or("unknown")
        ),
        String::new(),
        "Read this file at session start and after every compaction. \
         Do not repeat failed approaches listed below."
            .into(),
        String::new(),
    ];

    lines.extend(goal_section(&state));
    lines.extend(progress_section(&state));
    lines.extend(decisions_section(&state, config.snapshot_max_decisions as usize));
    lines.extend(failures_section(&attempts, config.snapshot_max_failures as usize));

    let mut text = lines.join("\n");
    if !text.ends_with('\n') {
        text.push('\n');
    }

    let max_lines = config.snapshot_max_lines as usize;
    let line_count = text.lines().count();
    if line_count > max_lines {
        let truncated: Vec<&str> = text.lines().take(max_lines.saturating_sub(1)).collect();
        text = truncated.join("\n") + "\n…(truncated)\n";
    }
    Ok(text)
}

fn goal_section(state: &crate::state::KeelState) -> Vec<String> {
    let Some(goal) = &state.goal else {
        return vec![
            "## Goal".into(),
            "_No active goal. Run `keel goal set \"...\"`._".into(),
            String::new(),
        ];
    };
    let mut lines = vec![
        "## Goal".into(),
        format!("**{}**", goal.title),
        String::new(),
    ];
    if !goal.acceptance.is_empty() {
        lines.push("### Acceptance".into());
        for item in goal.acceptance.iter().take(12) {
            lines.push(format!("- {item}"));
        }
        lines.push(String::new());
    }
    if !goal.constraints.is_empty() {
        lines.push("### Constraints".into());
        for item in goal.constraints.iter().take(12) {
            lines.push(format!("- {item}"));
        }
        lines.push(String::new());
    }
    lines
}

fn progress_section(state: &crate::state::KeelState) -> Vec<String> {
    let mut lines = vec!["## Progress".into(), String::new()];
    if let Some(step) = &state.progress.current_step {
        lines.push(format!("**Current step:** {step}"));
        lines.push(String::new());
    }
    if !state.progress.completed.is_empty() {
        lines.push("### Done".into());
        let start = state.progress.completed.len().saturating_sub(10);
        for item in &state.progress.completed[start..] {
            lines.push(format!("- {item}"));
        }
        lines.push(String::new());
    }
    if !state.progress.blockers.is_empty() {
        lines.push("### Blockers".into());
        let start = state.progress.blockers.len().saturating_sub(8);
        for item in &state.progress.blockers[start..] {
            lines.push(format!("- {item}"));
        }
        lines.push(String::new());
    }
    lines
}

fn decisions_section(state: &crate::state::KeelState, limit: usize) -> Vec<String> {
    if state.decisions.is_empty() {
        return vec![];
    }
    let mut lines = vec!["## Recent decisions".into(), String::new()];
    let start = state.decisions.len().saturating_sub(limit);
    for d in &state.decisions[start..] {
        lines.push(format!("- {}", d.text));
    }
    lines.push(String::new());
    lines
}

fn failures_section(attempts: &[serde_json::Value], limit: usize) -> Vec<String> {
    let failures: Vec<&serde_json::Value> = attempts.iter().filter(|a| a["ok"] == false).collect();
    if failures.is_empty() {
        return vec![
            "## Do NOT retry".into(),
            "_No recorded failures yet._".into(),
            String::new(),
        ];
    }

    let mut seen: HashMap<String, &serde_json::Value> = HashMap::new();
    for f in failures {
        let tool = f["tool"].as_str().unwrap_or("?");
        let action = f["action"].as_str().unwrap_or("");
        seen.insert(format!("{tool}::{action}"), f);
    }

    let mut lines = vec![
        "## Do NOT retry (already failed)".into(),
        "_These approaches failed. Try a different strategy._".into(),
        String::new(),
    ];
    let items: Vec<_> = seen.values().collect();
    let start = items.len().saturating_sub(limit);
    for f in &items[start..] {
        let tool = f["tool"].as_str().unwrap_or("?");
        let action = f["action"].as_str().unwrap_or("");
        let action_short: String = action.chars().take(120).collect();
        lines.push(format!("- **{tool}:** `{action_short}`"));
        if let Some(detail) = f["detail"].as_str() {
            let d: String = detail.chars().take(200).collect();
            if !d.is_empty() {
                lines.push(format!("  - {d}"));
            }
        }
        if let Some(code) = f["exit_code"].as_i64() {
            if code != 0 {
                lines.push(format!("  - exit code: {code}"));
            }
        }
    }
    lines.push(String::new());
    lines
}

pub fn write_snapshot(root: Option<&Path>) -> Result<std::path::PathBuf> {
    let path = keel_dir(root).join(SNAPSHOT_FILE);
    let text = render_snapshot(root)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, text)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{Goal, KeelState};

    #[test]
    fn snapshot_includes_goal() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join(crate::KEEL_DIR)).unwrap();

        let mut state = KeelState::default();
        state.goal = Some(Goal {
            title: "Ship v0.2".into(),
            acceptance: vec!["tests pass".into()],
            constraints: vec![],
            started_at: crate::paths::utcnow(),
        });
        state.progress.current_step = Some("write rust".into());
        crate::state::save_state(&mut state, Some(root)).unwrap();
        crate::state::save_config(&crate::state::KeelConfig::default(), Some(root)).unwrap();

        let snap = render_snapshot(Some(root)).unwrap();
        assert!(snap.contains("Ship v0.2"));
        assert!(snap.contains("write rust"));
    }
}
