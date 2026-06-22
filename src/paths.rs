use anyhow::{Context, Result};
use chrono::{SecondsFormat, Utc};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::KEEL_DIR;

pub const STATE_FILE: &str = "state.json";
pub const CONFIG_FILE: &str = "config.json";
pub const CHANGELOG_FILE: &str = "changelog.jsonl";
pub const ATTEMPTS_FILE: &str = "attempts.jsonl";
pub const SNAPSHOT_FILE: &str = "snapshot.md";

pub fn utcnow() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

pub fn find_project_root(start: Option<&Path>) -> PathBuf {
    let start = start
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let start = start.canonicalize().unwrap_or(start);
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .and_then(|h| h.canonicalize().ok());

    for dir in start.ancestors() {
        if dir.join(".git").exists() {
            return dir.to_path_buf();
        }
        if dir.join(KEEL_DIR).is_dir() {
            // Ignore ~/.keel when working in a subdirectory (common mistake after
            // `keel cloud link` run from $HOME).
            let is_home_keel = home.as_ref().is_some_and(|h| h == dir);
            if !is_home_keel {
                return dir.to_path_buf();
            }
        }
    }
    start
}

pub fn keel_dir(root: Option<&Path>) -> PathBuf {
    find_project_root(root).join(KEEL_DIR)
}

pub fn ensure_keel_dir(root: Option<&Path>) -> Result<PathBuf> {
    let dir = keel_dir(root);
    fs::create_dir_all(&dir).with_context(|| format!("create {}", dir.display()))?;
    Ok(dir)
}

pub fn write_json_atomic(path: &Path, value: &serde_json::Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let data = serde_json::to_string_pretty(value)? + "\n";
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, &data)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

pub fn read_json(path: &Path, default: serde_json::Value) -> Result<serde_json::Value> {
    if !path.exists() {
        return Ok(default);
    }
    let raw = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&raw)?)
}

pub fn append_jsonl(path: &Path, record: &serde_json::Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    writeln!(file, "{}", serde_json::to_string(record)?)?;
    Ok(())
}

pub fn read_jsonl_tail(path: &Path, limit: usize) -> Result<Vec<serde_json::Value>> {
    if !path.exists() {
        return Ok(vec![]);
    }
    let raw = fs::read_to_string(path)?;
    let lines: Vec<&str> = raw.lines().filter(|l| !l.trim().is_empty()).collect();
    let start = lines.len().saturating_sub(limit);
    let mut out = Vec::new();
    for line in &lines[start..] {
        out.push(serde_json::from_str(line)?);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_json_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("test.json");
        let v = serde_json::json!({"a": 1});
        write_json_atomic(&path, &v).unwrap();
        let back = read_json(&path, serde_json::json!(null)).unwrap();
        assert_eq!(back["a"], 1);
    }

    #[test]
    fn ignores_home_keel_for_subdirs() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("home");
        let project = home.join("myapp");
        std::fs::create_dir_all(home.join(KEEL_DIR)).unwrap();
        std::fs::create_dir_all(&project).unwrap();

        let old_home = std::env::var_os("HOME");
        std::env::set_var("HOME", &home);

        let root = find_project_root(Some(&project));
        assert_eq!(root, project.canonicalize().unwrap());

        if let Some(h) = old_home {
            std::env::set_var("HOME", h);
        } else {
            std::env::remove_var("HOME");
        }
    }
}
