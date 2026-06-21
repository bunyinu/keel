use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub api_key: String,
    pub state_json: String,
    pub snapshot_md: String,
    pub updated_at: String,
}

static DB: OnceLock<Mutex<Connection>> = OnceLock::new();

pub fn init_db(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(path).context("open database")?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS projects (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            api_key TEXT NOT NULL UNIQUE,
            state_json TEXT NOT NULL DEFAULT '{}',
            snapshot_md TEXT NOT NULL DEFAULT '',
            updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_projects_api_key ON projects(api_key);",
    )?;
    DB.set(Mutex::new(conn))
        .map_err(|_| anyhow::anyhow!("database already initialized"))?;
    Ok(())
}

fn conn() -> Result<std::sync::MutexGuard<'static, Connection>> {
    DB.get()
        .context("database not initialized")?
        .lock()
        .map_err(|e| anyhow::anyhow!("db lock poisoned: {e}"))
}

fn new_api_key() -> String {
    format!("keel_{}", Uuid::new_v4().simple())
}

pub fn create_project(name: &str) -> Result<Project> {
    let id = Uuid::new_v4().to_string();
    let api_key = new_api_key();
    let now = chrono::Utc::now().to_rfc3339();
    let c = conn()?;
    c.execute(
        "INSERT INTO projects (id, name, api_key, state_json, snapshot_md, updated_at)
         VALUES (?1, ?2, ?3, '{}', '', ?4)",
        params![id, name, api_key, now],
    )?;
    Ok(Project {
        id,
        name: name.to_string(),
        api_key,
        state_json: "{}".into(),
        snapshot_md: String::new(),
        updated_at: now,
    })
}

pub fn get_by_api_key(api_key: &str) -> Result<Option<Project>> {
    let c = conn()?;
    let mut stmt = c.prepare(
        "SELECT id, name, api_key, state_json, snapshot_md, updated_at
         FROM projects WHERE api_key = ?1",
    )?;
    let mut rows = stmt.query(params![api_key])?;
    if let Some(row) = rows.next()? {
        return Ok(Some(Project {
            id: row.get(0)?,
            name: row.get(1)?,
            api_key: row.get(2)?,
            state_json: row.get(3)?,
            snapshot_md: row.get(4)?,
            updated_at: row.get(5)?,
        }));
    }
    Ok(None)
}

pub fn get_by_id(id: &str) -> Result<Option<Project>> {
    let c = conn()?;
    let mut stmt = c.prepare(
        "SELECT id, name, api_key, state_json, snapshot_md, updated_at
         FROM projects WHERE id = ?1",
    )?;
    let mut rows = stmt.query(params![id])?;
    if let Some(row) = rows.next()? {
        return Ok(Some(Project {
            id: row.get(0)?,
            name: row.get(1)?,
            api_key: row.get(2)?,
            state_json: row.get(3)?,
            snapshot_md: row.get(4)?,
            updated_at: row.get(5)?,
        }));
    }
    Ok(None)
}

pub fn sync_project(
    id: &str,
    state_json: &str,
    snapshot_md: &str,
) -> Result<()> {
    let now = chrono::Utc::now().to_rfc3339();
    let c = conn()?;
    let n = c.execute(
        "UPDATE projects SET state_json = ?1, snapshot_md = ?2, updated_at = ?3 WHERE id = ?4",
        params![state_json, snapshot_md, now, id],
    )?;
    if n == 0 {
        anyhow::bail!("project not found");
    }
    Ok(())
}

pub fn list_projects(limit: usize) -> Result<Vec<Project>> {
    let c = conn()?;
    let mut stmt = c.prepare(
        "SELECT id, name, api_key, state_json, snapshot_md, updated_at
         FROM projects ORDER BY updated_at DESC LIMIT ?1",
    )?;
    let rows = stmt.query_map(params![limit as i64], |row| {
        Ok(Project {
            id: row.get(0)?,
            name: row.get(1)?,
            api_key: row.get(2)?,
            state_json: row.get(3)?,
            snapshot_md: row.get(4)?,
            updated_at: row.get(5)?,
        })
    })?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}
