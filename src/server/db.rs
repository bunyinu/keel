use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use uuid::Uuid;

pub const PLAN_FREE: &str = "free";
pub const PLAN_PRO: &str = "pro";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Team {
    pub id: String,
    pub name: String,
    pub plan: String,
    pub license_key: String,
    pub max_projects: i32,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub api_key: String,
    pub team_id: String,
    pub state_json: String,
    pub snapshot_md: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSummary {
    pub id: String,
    pub name: String,
    pub updated_at: String,
    pub dashboard_url: String,
    /// Active goal title from synced state (empty if unset).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub goal_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_step: Option<String>,
    pub compactions: u32,
}

static DB: OnceLock<Mutex<Connection>> = OnceLock::new();

pub fn free_project_limit() -> i32 {
    std::env::var("KEEL_FREE_PROJECT_LIMIT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1)
}

pub fn pro_project_limit() -> i32 {
    std::env::var("KEEL_PRO_PROJECT_LIMIT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(50)
}

pub fn init_db(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(path).context("open database")?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS teams (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            plan TEXT NOT NULL DEFAULT 'free',
            license_key TEXT NOT NULL UNIQUE,
            max_projects INTEGER NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS projects (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            api_key TEXT NOT NULL UNIQUE,
            team_id TEXT,
            state_json TEXT NOT NULL DEFAULT '{}',
            snapshot_md TEXT NOT NULL DEFAULT '',
            updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_projects_api_key ON projects(api_key);
        CREATE INDEX IF NOT EXISTS idx_projects_team_id ON projects(team_id);
        CREATE INDEX IF NOT EXISTS idx_teams_license ON teams(license_key);",
    )?;
    migrate_schema(&conn)?;
    DB.set(Mutex::new(conn))
        .map_err(|_| anyhow::anyhow!("database already initialized"))?;
    migrate_orphan_projects()?;
    Ok(())
}

fn migrate_schema(conn: &Connection) -> Result<()> {
    let mut stmt = conn.prepare("PRAGMA table_info(projects)")?;
    let cols: Vec<String> = stmt
        .query_map([], |row| row.get(1))?
        .collect::<Result<Vec<_>, _>>()?;
    if !cols.iter().any(|c| c == "team_id") {
        conn.execute("ALTER TABLE projects ADD COLUMN team_id TEXT", [])?;
    }
    Ok(())
}

fn migrate_orphan_projects() -> Result<()> {
    let c = conn()?;
    let mut stmt = c.prepare("SELECT id, name FROM projects WHERE team_id IS NULL OR team_id = ''")?;
    let orphans: Vec<(String, String)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<Vec<_>, _>>()?;
    drop(stmt);
    for (pid, pname) in orphans {
        let team = create_team_internal(&pname, PLAN_FREE, free_project_limit())?;
        c.execute(
            "UPDATE projects SET team_id = ?1 WHERE id = ?2",
            params![team.id, pid],
        )?;
    }
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

fn new_team_license() -> String {
    format!("keel_team_{}", Uuid::new_v4().simple())
}

fn create_team_internal(name: &str, plan: &str, max_projects: i32) -> Result<Team> {
    let id = Uuid::new_v4().to_string();
    let license_key = new_team_license();
    let now = chrono::Utc::now().to_rfc3339();
    let c = conn()?;
    c.execute(
        "INSERT INTO teams (id, name, plan, license_key, max_projects, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![id, name, plan, license_key, max_projects, now],
    )?;
    Ok(Team {
        id,
        name: name.to_string(),
        plan: plan.to_string(),
        license_key,
        max_projects,
        created_at: now,
    })
}

pub fn create_team(name: &str) -> Result<Team> {
    create_team_internal(name, PLAN_FREE, free_project_limit())
}

pub fn get_team_by_license(license_key: &str) -> Result<Option<Team>> {
    let c = conn()?;
    let mut stmt = c.prepare(
        "SELECT id, name, plan, license_key, max_projects, created_at FROM teams WHERE license_key = ?1",
    )?;
    let mut rows = stmt.query(params![license_key])?;
    if let Some(row) = rows.next()? {
        return Ok(Some(Team {
            id: row.get(0)?,
            name: row.get(1)?,
            plan: row.get(2)?,
            license_key: row.get(3)?,
            max_projects: row.get(4)?,
            created_at: row.get(5)?,
        }));
    }
    Ok(None)
}

pub fn get_team_by_id(id: &str) -> Result<Option<Team>> {
    let c = conn()?;
    let mut stmt = c.prepare(
        "SELECT id, name, plan, license_key, max_projects, created_at FROM teams WHERE id = ?1",
    )?;
    let mut rows = stmt.query(params![id])?;
    if let Some(row) = rows.next()? {
        return Ok(Some(Team {
            id: row.get(0)?,
            name: row.get(1)?,
            plan: row.get(2)?,
            license_key: row.get(3)?,
            max_projects: row.get(4)?,
            created_at: row.get(5)?,
        }));
    }
    Ok(None)
}

pub fn count_team_projects(team_id: &str) -> Result<i32> {
    let c = conn()?;
    let n: i32 = c.query_row(
        "SELECT COUNT(*) FROM projects WHERE team_id = ?1",
        params![team_id],
        |row| row.get(0),
    )?;
    Ok(n)
}

pub fn upgrade_team_to_pro(team_license: &str) -> Result<Team> {
    let team = get_team_by_license(team_license)?
        .ok_or_else(|| anyhow::anyhow!("team not found"))?;
    let now = chrono::Utc::now().to_rfc3339();
    let max = pro_project_limit();
    let c = conn()?;
    c.execute(
        "UPDATE teams SET plan = ?1, max_projects = ?2 WHERE id = ?3",
        params![PLAN_PRO, max, team.id],
    )?;
    Ok(Team {
        plan: PLAN_PRO.into(),
        max_projects: max,
        created_at: now,
        ..team
    })
}

pub fn create_project(name: &str, team_license: Option<&str>) -> Result<Project> {
    let team = if let Some(key) = team_license {
        let team = get_team_by_license(key)?.ok_or_else(|| anyhow::anyhow!("invalid team license"))?;
        let count = count_team_projects(&team.id)?;
        if count >= team.max_projects {
            anyhow::bail!("project limit reached for {} plan ({})", team.plan, team.max_projects);
        }
        team
    } else {
        create_team(name)?
    };

    let id = Uuid::new_v4().to_string();
    let api_key = new_api_key();
    let now = chrono::Utc::now().to_rfc3339();
    let c = conn()?;
    c.execute(
        "INSERT INTO projects (id, name, api_key, team_id, state_json, snapshot_md, updated_at)
         VALUES (?1, ?2, ?3, ?4, '{}', '', ?5)",
        params![id, name, api_key, team.id, now],
    )?;
    Ok(Project {
        id,
        name: name.to_string(),
        api_key,
        team_id: team.id,
        state_json: "{}".into(),
        snapshot_md: String::new(),
        updated_at: now,
    })
}

pub fn list_team_projects(team_id: &str) -> Result<Vec<ProjectSummary>> {
    let c = conn()?;
    let mut stmt = c.prepare(
        "SELECT id, name, updated_at, state_json FROM projects WHERE team_id = ?1 ORDER BY updated_at DESC",
    )?;
    let rows = stmt.query_map(params![team_id], |row| {
        let id: String = row.get(0)?;
        let state_json: String = row.get(3)?;
        let (goal_title, current_step, compactions) = fleet_fields_from_state(&state_json);
        Ok(ProjectSummary {
            id: id.clone(),
            name: row.get(1)?,
            updated_at: row.get(2)?,
            dashboard_url: format!("/dashboard/{id}"),
            goal_title,
            current_step,
            compactions,
        })
    })?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

fn fleet_fields_from_state(state_json: &str) -> (Option<String>, Option<String>, u32) {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(state_json) else {
        return (None, None, 0);
    };
    let goal_title = v
        .pointer("/goal/title")
        .and_then(|t| t.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    let current_step = v
        .pointer("/progress/current_step")
        .and_then(|t| t.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    let compactions = v
        .get("compactions")
        .and_then(|c| c.as_u64())
        .unwrap_or(0) as u32;
    (goal_title, current_step, compactions)
}

pub fn get_by_api_key(api_key: &str) -> Result<Option<Project>> {
    let c = conn()?;
    let mut stmt = c.prepare(
        "SELECT id, name, api_key, team_id, state_json, snapshot_md, updated_at
         FROM projects WHERE api_key = ?1",
    )?;
    let mut rows = stmt.query(params![api_key])?;
    if let Some(row) = rows.next()? {
        return Ok(Some(Project {
            id: row.get(0)?,
            name: row.get(1)?,
            api_key: row.get(2)?,
            team_id: row.get(3)?,
            state_json: row.get(4)?,
            snapshot_md: row.get(5)?,
            updated_at: row.get(6)?,
        }));
    }
    Ok(None)
}

pub fn get_by_id(id: &str) -> Result<Option<Project>> {
    let c = conn()?;
    let mut stmt = c.prepare(
        "SELECT id, name, api_key, team_id, state_json, snapshot_md, updated_at
         FROM projects WHERE id = ?1",
    )?;
    let mut rows = stmt.query(params![id])?;
    if let Some(row) = rows.next()? {
        return Ok(Some(Project {
            id: row.get(0)?,
            name: row.get(1)?,
            api_key: row.get(2)?,
            team_id: row.get(3)?,
            state_json: row.get(4)?,
            snapshot_md: row.get(5)?,
            updated_at: row.get(6)?,
        }));
    }
    Ok(None)
}

pub fn sync_project(id: &str, state_json: &str, snapshot_md: &str) -> Result<()> {
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
        "SELECT id, name, api_key, team_id, state_json, snapshot_md, updated_at
         FROM projects ORDER BY updated_at DESC LIMIT ?1",
    )?;
    let rows = stmt.query_map(params![limit as i64], |row| {
        Ok(Project {
            id: row.get(0)?,
            name: row.get(1)?,
            api_key: row.get(2)?,
            team_id: row.get(3)?,
            state_json: row.get(4)?,
            snapshot_md: row.get(5)?,
            updated_at: row.get(6)?,
        })
    })?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

pub fn valid_upgrade_code(code: &str) -> bool {
    let code = code.trim();
    if code.is_empty() {
        return false;
    }
    std::env::var("KEEL_UPGRADE_CODES")
        .ok()
        .map(|raw| raw.split(',').map(str::trim).any(|c| c == code))
        .unwrap_or_else(|| code.starts_with("keel_pro_"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn billing_tier_limits() {
        std::env::set_var("KEEL_UPGRADE_CODES", "testcode");
        let tmp = tempfile::tempdir().unwrap();
        init_db(&tmp.path().join("test.db")).unwrap();
        let p1 = create_project("one", None).unwrap();
        let team = get_team_by_id(&p1.team_id).unwrap().unwrap();
        let err = create_project("two", Some(&team.license_key)).unwrap_err();
        assert!(err.to_string().contains("project limit"));
        upgrade_team_to_pro(&team.license_key).unwrap();
        let p2 = create_project("two", Some(&team.license_key)).unwrap();
        assert_ne!(p1.id, p2.id);
        std::env::remove_var("KEEL_UPGRADE_CODES");
    }
}
