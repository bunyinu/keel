use anyhow::Result;
use serde_json::Value;
use std::path::{Path, PathBuf};

use crate::install::keel_binary;
use crate::paths::{find_project_root, keel_dir};
use crate::state::{load_config, load_state};
use crate::VERSION;

pub struct Check {
    pub ok: bool,
    pub label: String,
    pub detail: String,
}

pub fn run_doctor() -> Result<Vec<Check>> {
    let mut checks = Vec::new();

    checks.push(Check {
        ok: true,
        label: "Keel version".into(),
        detail: VERSION.into(),
    });

    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let root = find_project_root(None);
    let keel_path = keel_dir(None);
    let has_config = keel_path.join("config.json").exists();
    let partial_keel = keel_path.is_dir() && !has_config;

    let home_keel = std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|h| h.join(".keel"))
        .filter(|p| p.is_dir());
    let cwd_canon = cwd.canonicalize().unwrap_or(cwd.clone());
    let home_canon = std::env::var_os("HOME")
        .and_then(|h| PathBuf::from(h).canonicalize().ok());
    let mislinked_home = home_keel.is_some()
        && home_canon.as_ref() != Some(&cwd_canon)
        && keel_path == home_canon.as_ref().map(|h| h.join(".keel")).unwrap_or_default()
        && !cwd.join(".keel").exists()
        && !cwd.join(".git").exists();

    checks.push(Check {
        ok: has_config,
        label: ".keel initialized".into(),
        detail: if has_config {
            format!("{}", keel_path.display())
        } else if partial_keel {
            format!(
                "partial .keel at {} — run `keel onboard \"...\"` or `keel init`",
                keel_path.display()
            )
        } else {
            "Run `keel onboard \"your task\" --accept \"tests pass\"`".into()
        },
    });

    checks.push(Check {
        ok: !mislinked_home,
        label: "Project root".into(),
        detail: if mislinked_home {
            "Looks like ~/.keel is being used — run `cd your-repo` then `keel init`".into()
        } else {
            format!("{}", root.display())
        },
    });

    let codex_path = root.join(".codex");
    let codex_ok = !codex_path.exists() || codex_path.is_dir();
    checks.push(Check {
        ok: codex_ok,
        label: ".codex directory".into(),
        detail: if codex_ok {
            "OK".into()
        } else {
            format!(
                "{} is a file — rename it so Codex hooks can install",
                codex_path.display()
            )
        },
    });

    let claude_hooks = root.join(".claude/settings.json");
    let (claude_ok, claude_detail) = hooks_contain_keel(&claude_hooks);
    checks.push(Check {
        ok: claude_ok,
        label: "Claude Code hooks".into(),
        detail: claude_detail,
    });

    let codex_hooks = root.join(".codex/hooks.json");
    let (codex_ok, codex_detail) = hooks_contain_keel(&codex_hooks);
    checks.push(Check {
        ok: codex_ok,
        label: "Codex hooks".into(),
        detail: codex_detail,
    });

    let cursor_hooks = root.join(".cursor/hooks.json");
    let (cursor_ok, cursor_detail) = hooks_contain_keel_cursor(&cursor_hooks);
    checks.push(Check {
        ok: cursor_ok,
        label: "Cursor hooks".into(),
        detail: cursor_detail,
    });

    let hooks_installed = claude_ok || codex_ok || cursor_ok;
    let cloud_ok = keel_path.join("cloud.json").exists();
    checks.push(Check {
        ok: true,
        label: "Keel Cloud link".into(),
        detail: if cloud_ok {
            "cloud.json present".into()
        } else {
            "optional — `keel cloud link ...`".into()
        },
    });

    let state = load_state(None).ok();
    let has_goal = state
        .as_ref()
        .and_then(|s| s.goal.as_ref())
        .is_some_and(|g| !g.title.trim().is_empty());
    checks.push(Check {
        ok: !hooks_installed || has_goal,
        label: "Active goal".into(),
        detail: if has_goal {
            state
                .as_ref()
                .and_then(|s| s.goal.as_ref())
                .map(|g| g.title.clone())
                .unwrap_or_default()
        } else if hooks_installed {
            "required when hooks are installed — run `keel onboard \"...\"`".into()
        } else {
            "optional — `keel onboard \"...\"` or `keel tui`".into()
        },
    });

    let config = load_config(None).ok();
    let gate = config.as_ref().map(|c| &c.acceptance_gate);
    let gate_on = gate.is_some_and(|g| g.enabled && !g.command.trim().is_empty());
    checks.push(Check {
        ok: true,
        label: "Acceptance gate".into(),
        detail: if gate_on {
            format!("enabled: `{}`", gate.unwrap().command)
        } else {
            "off — `keel config set --acceptance \"npm test\"`".into()
        },
    });

    let expected_bin = keel_binary();
    checks.push(Check {
        ok: true,
        label: "Hook binary".into(),
        detail: expected_bin,
    });

    Ok(checks)
}

fn hooks_contain_keel_cursor(path: &Path) -> (bool, String) {
    if !path.exists() {
        return (false, format!("missing {} — run `keel init`", path.display()));
    }
    let raw = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => return (false, e.to_string()),
    };
    if raw.contains("keel hook") {
        (true, path.display().to_string())
    } else {
        (false, "no keel hooks — run `keel init`".into())
    }
}

fn hooks_contain_keel(path: &Path) -> (bool, String) {
    if !path.exists() {
        return (false, format!("missing {}", path.display()));
    }
    let raw = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => return (false, e.to_string()),
    };
    let doc: Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => return (false, format!("invalid JSON: {e}")),
    };
    let text = doc.to_string();
    if text.contains("keel hook") {
        (true, path.display().to_string())
    } else {
        (false, "no keel hooks — run `keel init`".into())
    }
}

pub fn print_report(checks: &[Check]) -> bool {
    let mut all_ok = true;
    for c in checks {
        let icon = if c.ok { "✓" } else { "✗" };
        if !c.ok {
            all_ok = false;
        }
        println!("{icon} {} — {}", c.label, c.detail);
    }
    all_ok
}
