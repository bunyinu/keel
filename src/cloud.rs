use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::paths::keel_dir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudConfig {
    pub url: String,
    pub project_id: String,
    pub api_key: String,
}

pub fn cloud_config_path(root: Option<&Path>) -> std::path::PathBuf {
    keel_dir(root).join("cloud.json")
}

pub fn load_cloud_config(root: Option<&Path>) -> Result<Option<CloudConfig>> {
    let path = cloud_config_path(root);
    if !path.exists() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(&path)?;
    Ok(Some(serde_json::from_str(&raw)?))
}

pub fn save_cloud_config(config: &CloudConfig, root: Option<&Path>) -> Result<()> {
    let path = cloud_config_path(root);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_string_pretty(config)? + "\n")?;
    Ok(())
}

pub fn push_state(root: Option<&Path>) -> Result<()> {
    let Some(config) = load_cloud_config(root)? else {
        return Ok(());
    };
    let state_path = keel_dir(root).join(crate::paths::STATE_FILE);
    let snapshot_path = keel_dir(root).join(crate::paths::SNAPSHOT_FILE);
    let state_json = std::fs::read_to_string(&state_path).unwrap_or_else(|_| "{}".into());
    let snapshot_md = std::fs::read_to_string(&snapshot_path).unwrap_or_default();

    let url = format!(
        "{}/api/projects/{}/sync",
        config.url.trim_end_matches('/'),
        config.project_id
    );
    let body = serde_json::json!({
        "state": serde_json::from_str::<serde_json::Value>(&state_json).unwrap_or(serde_json::json!({})),
        "snapshot": snapshot_md,
    });

    let resp = ureq::post(&url)
        .set("Authorization", &format!("Bearer {}", config.api_key))
        .send_json(body)?;

    if resp.status() >= 400 {
        anyhow::bail!("cloud sync failed: HTTP {}", resp.status());
    }
    Ok(())
}

pub fn pull_state(root: Option<&Path>) -> Result<()> {
    let Some(config) = load_cloud_config(root)? else {
        return Ok(());
    };
    let url = format!(
        "{}/api/projects/{}",
        config.url.trim_end_matches('/'),
        config.project_id
    );
    let resp = ureq::get(&url)
        .set("Authorization", &format!("Bearer {}", config.api_key))
        .call()
        .context("cloud pull request")?;

    if resp.status() >= 400 {
        anyhow::bail!("cloud pull failed: HTTP {}", resp.status());
    }

    let body: serde_json::Value = resp.into_json()?;
    let keel = keel_dir(root);
    std::fs::create_dir_all(&keel)?;

    if let Some(state) = body.get("state") {
        crate::paths::write_json_atomic(&keel.join(crate::paths::STATE_FILE), state)?;
    }
    if let Some(snapshot) = body.get("snapshot").and_then(|s| s.as_str()) {
        std::fs::write(keel.join(crate::paths::SNAPSHOT_FILE), snapshot)?;
    }
    Ok(())
}
