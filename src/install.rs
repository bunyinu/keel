use anyhow::Result;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

use crate::paths::{ensure_keel_dir, find_project_root};
use crate::snapshot::write_snapshot;
use crate::state::init_config;

const CLAUDE_MD_SNIPPET: &str = r#"## Keel (agent state)

At the start of every session and after compaction, read `.keel/snapshot.md` before making changes.
Keel tracks the active goal, progress, decisions, and failed attempts. Do not repeat actions listed under "Do NOT retry".
Update progress with `keel progress --step "..."` or `keel progress --done "..."` when appropriate.
"#;

const AGENTS_MD_SNIPPET: &str = r#"## Keel (agent state)

At session start and after compaction, read `.keel/snapshot.md` before editing files or running commands.
Do not repeat failed approaches listed there. Use `keel goal set` / `keel progress` to keep state current.
"#;

pub fn keel_binary() -> String {
    if let Ok(bin) = std::env::var("KEEL_BIN") {
        if bin.contains(' ') {
            return format!("\"{bin}\"");
        }
        return bin;
    }

    if let Some(path) = find_keel_on_path() {
        let s = path.display().to_string();
        return if s.contains(' ') {
            format!("\"{s}\"")
        } else {
            s
        };
    }

    if let Ok(exe) = std::env::current_exe() {
        let name = exe.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name == "keel" || name == "keel.exe" {
            let s = exe.display().to_string();
            return if s.contains(' ') {
                format!("\"{s}\"")
            } else {
                s
            };
        }
    }

    "keel".to_string()
}

fn find_keel_on_path() -> Option<PathBuf> {
    let output = std::process::Command::new("sh")
        .args(["-c", "command -v keel"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if path.is_empty() {
        None
    } else {
        Some(PathBuf::from(path))
    }
}

fn hook_cmd(event: &str, agent: &str) -> String {
    format!("{} hook {event} --agent {agent}", keel_binary())
}

fn claude_hooks() -> Value {
    json!({
        "PreCompact": [{
            "matcher": "",
            "hooks": [{"type": "command", "command": hook_cmd("pre-compact", "claude"), "timeout": 15}]
        }],
        "SessionStart": [{
            "matcher": "compact|resume",
            "hooks": [{"type": "command", "command": hook_cmd("session-start", "claude"), "timeout": 15}]
        }],
        "PreToolUse": [{
            "matcher": "Bash|Edit|Write|ApplyPatch",
            "hooks": [{"type": "command", "command": hook_cmd("pre-tool-use", "claude"), "timeout": 10}]
        }],
        "PostToolUse": [{
            "matcher": "Bash|Edit|Write|ApplyPatch",
            "hooks": [{"type": "command", "command": hook_cmd("post-tool-use", "claude"), "timeout": 10}]
        }]
    })
}

fn codex_hooks() -> Value {
    json!({
        "PreCompact": [{
            "matcher": "manual|auto",
            "hooks": [{
                "type": "command",
                "command": hook_cmd("pre-compact", "codex"),
                "timeout": 15,
                "statusMessage": "Keel: saving state before compaction"
            }]
        }],
        "PostCompact": [{
            "matcher": "manual|auto",
            "hooks": [{
                "type": "command",
                "command": hook_cmd("post-compact", "codex"),
                "timeout": 15,
                "statusMessage": "Keel: restoring state after compaction"
            }]
        }],
        "SessionStart": [{
            "matcher": "startup|resume|clear|compact",
            "hooks": [{
                "type": "command",
                "command": hook_cmd("session-start", "codex"),
                "timeout": 15,
                "statusMessage": "Keel: loading session state"
            }]
        }],
        "PreToolUse": [{
            "matcher": "Bash|apply_patch|Edit|Write",
            "hooks": [{
                "type": "command",
                "command": hook_cmd("pre-tool-use", "codex"),
                "timeout": 10,
                "statusMessage": "Keel: checking retry loop"
            }]
        }],
        "PostToolUse": [{
            "matcher": "Bash|apply_patch|Edit|Write",
            "hooks": [{"type": "command", "command": hook_cmd("post-tool-use", "codex"), "timeout": 10}]
        }],
        "UserPromptSubmit": [{
            "hooks": [{"type": "command", "command": hook_cmd("user-prompt-submit", "codex"), "timeout": 5}]
        }]
    })
}

fn merge_hooks(existing: &mut Value, new_hooks: &Value) {
    let hooks = existing
        .as_object_mut()
        .and_then(|o| o.entry("hooks").or_insert(json!({})).as_object_mut());

    let Some(hooks) = hooks else { return };
    let Some(new_obj) = new_hooks.as_object() else { return };

    for (event, groups) in new_obj {
        let current = hooks
            .entry(event.clone())
            .or_insert(json!([]))
            .as_array_mut()
            .unwrap();

        let keel_markers: Vec<String> = groups
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|g| {
                        g["hooks"][0]["command"]
                            .as_str()
                            .map(|s| s.to_string())
                    })
                    .collect()
            })
            .unwrap_or_default();

        current.retain(|g| {
            !g["hooks"]
                .as_array()
                .map(|hooks| {
                    hooks.iter().any(|h| {
                        h["command"]
                            .as_str()
                            .map(|c| c.contains("keel hook") || keel_markers.iter().any(|m| c == m))
                            .unwrap_or(false)
                    })
                })
                .unwrap_or(false)
        });

        if let Some(new_groups) = groups.as_array() {
            for g in new_groups {
                current.push(g.clone());
            }
        }
    }
}

fn append_snippet(path: &Path, snippet: &str, marker: &str) -> Result<()> {
    let text = if path.exists() {
        let existing = fs::read_to_string(path)?;
        if existing.contains(marker) || existing.contains(&snippet[..snippet.len().min(40)]) {
            return Ok(());
        }
        format!("{}\n\n{snippet}\n", existing.trim_end())
    } else {
        format!("{snippet}\n")
    };
    fs::write(path, text)?;
    Ok(())
}

pub fn install(project: Option<&Path>) -> Result<PathBuf> {
    let root = find_project_root(project);
    ensure_keel_dir(Some(&root))?;
    init_config(Some(&root))?;

    let claude_dir = root.join(".claude");
    fs::create_dir_all(&claude_dir)?;
    let settings_path = claude_dir.join("settings.json");
    let mut settings: Value = if settings_path.exists() {
        serde_json::from_str(&fs::read_to_string(&settings_path)?)?
    } else {
        json!({})
    };
    merge_hooks(&mut settings, &claude_hooks());
    fs::write(
        &settings_path,
        serde_json::to_string_pretty(&settings)? + "\n",
    )?;

    let codex_dir = root.join(".codex");
    fs::create_dir_all(&codex_dir)?;
    let hooks_path = codex_dir.join("hooks.json");
    let mut codex_doc: Value = if hooks_path.exists() {
        serde_json::from_str(&fs::read_to_string(&hooks_path)?)?
    } else {
        json!({"hooks": {}})
    };
    if codex_doc.get("hooks").is_none() {
        codex_doc["hooks"] = json!({});
    }
    merge_hooks(&mut codex_doc, &codex_hooks());
    fs::write(&hooks_path, serde_json::to_string_pretty(&codex_doc)? + "\n")?;

    append_snippet(&root.join("CLAUDE.md"), CLAUDE_MD_SNIPPET, "## Keel")?;
    append_snippet(&root.join("AGENTS.md"), AGENTS_MD_SNIPPET, "## Keel")?;
    write_snapshot(Some(&root))?;

    Ok(root)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_creates_hooks() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::env::set_var("KEEL_BIN", "keel");
        install(Some(root)).unwrap();
        assert!(root.join(".keel").is_dir());
        assert!(root.join(".claude/settings.json").exists());
        assert!(root.join(".codex/hooks.json").exists());
        let settings: Value =
            serde_json::from_str(&std::fs::read_to_string(root.join(".claude/settings.json")).unwrap())
                .unwrap();
        let hooks = settings["hooks"]["PreToolUse"].as_array().unwrap();
        assert!(hooks[0]["hooks"][0]["command"]
            .as_str()
            .unwrap()
            .contains("keel hook"));
    }
}
