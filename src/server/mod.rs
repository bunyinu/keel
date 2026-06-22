pub mod db;

use axum::{
    extract::Path,
    http::{header, HeaderMap, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::{get, post, put},
    Json, Router,
};

use crate::goal_edit::{apply_form, GoalForm};
use crate::snapshot::render_from_parts;
use crate::state::{KeelConfig, KeelState};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use self::db::{
    count_team_projects, create_project, get_by_api_key, get_by_id, get_team_by_id,
    get_team_by_license, list_team_projects, sync_project, upgrade_team_to_pro,
    valid_upgrade_code, Project, Team,
};

#[derive(Clone)]
pub struct AppState {
    pub version: String,
    pub stripe_payment_link: String,
}

#[derive(Deserialize)]
pub struct CreateProjectRequest {
    name: String,
    #[serde(default)]
    team_license: Option<String>,
}

#[derive(Serialize)]
pub struct CreateProjectResponse {
    id: String,
    name: String,
    api_key: String,
    dashboard_url: String,
    team_id: String,
    team_license: String,
    plan: String,
    projects_used: i32,
    projects_max: i32,
}

#[derive(Deserialize)]
pub struct UpgradeRequest {
    team_license: String,
    code: String,
}

#[derive(Serialize)]
pub struct TeamView {
    id: String,
    name: String,
    plan: String,
    license: String,
    max_projects: i32,
}

#[derive(Deserialize)]
pub struct SyncRequest {
    state: Value,
    snapshot: String,
}

fn extract_bearer(headers: &HeaderMap) -> Option<String> {
    headers
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
        .map(|s| s.to_string())
}

async fn auth_project(headers: &HeaderMap, project_id: &str) -> Result<Project, StatusCode> {
    let key = extract_bearer(headers).ok_or(StatusCode::UNAUTHORIZED)?;
    let project = get_by_api_key(&key).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let project = project.ok_or(StatusCode::UNAUTHORIZED)?;
    if project.id != project_id {
        return Err(StatusCode::FORBIDDEN);
    }
    Ok(project)
}

async fn health() -> impl IntoResponse {
    Json(json!({"ok": true, "service": "keel-cloud"}))
}

async fn create_project_handler(
    axum::extract::State(state): axum::extract::State<AppState>,
    Json(body): Json<CreateProjectRequest>,
) -> Result<Response, StatusCode> {
    let name = body.name.trim();
    if name.is_empty() {
        return Ok((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Project name is required"})),
        )
            .into_response());
    }
    let project = match create_project(name, body.team_license.as_deref()) {
        Ok(p) => p,
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("project limit") {
                return Ok((
                    StatusCode::PAYMENT_REQUIRED,
                    Json(json!({
                        "error": msg,
                        "upgrade_url": "/pricing",
                        "stripe_url": state.stripe_payment_link,
                    })),
                )
                    .into_response());
            }
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
    let team = get_team_by_id(&project.team_id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    let used = count_team_projects(&team.id).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(CreateProjectResponse {
        id: project.id.clone(),
        name: project.name,
        api_key: project.api_key,
        dashboard_url: format!("/dashboard/{}", project.id),
        team_id: team.id,
        team_license: team.license_key,
        plan: team.plan,
        projects_used: used,
        projects_max: team.max_projects,
    })
    .into_response())
}

async fn upgrade_handler(Json(body): Json<UpgradeRequest>) -> Result<Json<Value>, StatusCode> {
    if !valid_upgrade_code(&body.code) {
        return Err(StatusCode::FORBIDDEN);
    }
    let team = upgrade_team_to_pro(&body.team_license).map_err(|_| StatusCode::BAD_REQUEST)?;
    Ok(Json(json!({
        "ok": true,
        "team": team_json(&team),
    })))
}

async fn team_projects_handler(headers: HeaderMap) -> Result<Json<Value>, StatusCode> {
    let key = extract_bearer(&headers).ok_or(StatusCode::UNAUTHORIZED)?;
    let team = get_team_by_license(&key).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let team = team.ok_or(StatusCode::UNAUTHORIZED)?;
    let projects = list_team_projects(&team.id).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({
        "team": team_json(&team),
        "projects": projects,
    })))
}

fn team_json(team: &Team) -> TeamView {
    TeamView {
        id: team.id.clone(),
        name: team.name.clone(),
        plan: team.plan.clone(),
        license: team.license_key.clone(),
        max_projects: team.max_projects,
    }
}

async fn pricing_page(axum::extract::State(state): axum::extract::State<AppState>) -> Html<String> {
    let mut html = include_str!("../../web/pricing.html").to_string();
    let link = html_escape(&state.stripe_payment_link);
    html = html.replace(
        "window.KEEL_STRIPE_PAYMENT_LINK || stripeDefault",
        &format!("\"{link}\" || stripeDefault"),
    );
    Html(html)
}

async fn team_page() -> Html<&'static str> {
    Html(include_str!("../../web/team.html"))
}

async fn get_project(
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<Value>, StatusCode> {
    let project = auth_project(&headers, &id).await?;
    let state: Value = serde_json::from_str(&project.state_json).unwrap_or(json!({}));
    Ok(Json(json!({
        "id": project.id,
        "name": project.name,
        "state": state,
        "snapshot": project.snapshot_md,
        "updated_at": project.updated_at,
    })))
}

async fn sync_handler(
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(body): Json<SyncRequest>,
) -> Result<StatusCode, StatusCode> {
    let _project = auth_project(&headers, &id).await?;
    let state_json = serde_json::to_string(&body.state).map_err(|_| StatusCode::BAD_REQUEST)?;
    sync_project(&id, &state_json, &body.snapshot)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Serialize)]
struct GoalResponse {
    snapshot: String,
    state: Value,
}

async fn update_goal_handler(
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(form): Json<GoalForm>,
) -> Result<Response, StatusCode> {
    if form.title.trim().is_empty() {
        return Ok((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Goal title is required"})),
        )
            .into_response());
    }
    let project = auth_project(&headers, &id).await?;
    let mut state: KeelState = serde_json::from_str(&project.state_json).unwrap_or_default();
    apply_form(&mut state, &form);
    let snapshot = render_from_parts(&state, &KeelConfig::default(), &[]);
    let state_json = serde_json::to_string(&state).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    sync_project(&id, &state_json, &snapshot).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let state_value: Value = serde_json::from_str(&state_json).unwrap_or(json!({}));
    Ok(Json(GoalResponse {
        snapshot,
        state: state_value,
    })
    .into_response())
}

async fn dashboard_edit(Path(id): Path<String>) -> Result<Html<String>, StatusCode> {
    let project = get_by_id(&id).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let project = project.ok_or(StatusCode::NOT_FOUND)?;
    let name = html_escape(&project.name);
    Ok(Html(format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>Keel — Edit goal — {name}</title>
  <style>
    :root {{ font-family: system-ui, sans-serif; background: #0f1419; color: #e7ecf3; }}
    body {{ max-width: 720px; margin: 0 auto; padding: 2rem 1rem; }}
    h1 {{ font-size: 1.5rem; }}
    label {{ display: block; margin: 1rem 0 .35rem; font-weight: 600; }}
    input, textarea {{ width: 100%; box-sizing: border-box; background: #1a2332; color: #e7ecf3; border: 1px solid #2d3a4f; border-radius: 6px; padding: .6rem; font: inherit; }}
    textarea {{ min-height: 5rem; resize: vertical; }}
    .row {{ display: flex; gap: .75rem; margin-top: 1.25rem; }}
    button, a.btn {{ background: #3d7dd6; color: #fff; border: none; border-radius: 6px; padding: .6rem 1rem; font: inherit; cursor: pointer; text-decoration: none; display: inline-block; }}
    a.btn.secondary {{ background: #243044; color: #e7ecf3; }}
    #status {{ margin-top: 1rem; color: #8b9bb4; }}
    .err {{ color: #ff8a8a; }}
    .ok {{ color: #7dcea0; }}
    code {{ background: #243044; padding: .15rem .4rem; border-radius: 4px; }}
  </style>
</head>
<body>
  <h1>Edit goal — {name}</h1>
  <p>Project <code>{id}</code></p>
  <label for="api_key">API key</label>
  <input id="api_key" type="password" placeholder="From project creation or dashboard owner" />
  <label for="title">Goal</label>
  <input id="title" type="text" placeholder="What are you building?" />
  <label for="step">Current step</label>
  <input id="step" type="text" placeholder="Optional" />
  <label for="acceptance">Acceptance criteria (one per line)</label>
  <textarea id="acceptance"></textarea>
  <label for="constraints">Constraints (one per line)</label>
  <textarea id="constraints"></textarea>
  <div class="row">
    <button type="button" id="save">Save goal</button>
    <a class="btn secondary" href="/dashboard/{id}">View dashboard</a>
  </div>
  <p id="status"></p>
  <script>
    const projectId = "{id}";
    const keyInput = document.getElementById("api_key");
    const status = document.getElementById("status");
    const storageKey = "keel_api_key_" + projectId;
    keyInput.value = localStorage.getItem(storageKey) || "";

    function lines(id) {{
      return document.getElementById(id).value.split("\\n").map(s => s.trim()).filter(Boolean);
    }}

    async function loadGoal() {{
      const key = keyInput.value.trim();
      if (!key) return;
      localStorage.setItem(storageKey, key);
      const res = await fetch("/api/projects/" + projectId, {{
        headers: {{ Authorization: "Bearer " + key }}
      }});
      if (!res.ok) {{ status.textContent = "Load failed: " + res.status; status.className = "err"; return; }}
      const data = await res.json();
      const g = data.state?.goal || {{}};
      document.getElementById("title").value = g.title || "";
      document.getElementById("step").value = data.state?.progress?.current_step || "";
      document.getElementById("acceptance").value = (g.acceptance || []).join("\\n");
      document.getElementById("constraints").value = (g.constraints || []).join("\\n");
      status.textContent = "Loaded current goal";
      status.className = "";
    }}

    document.getElementById("save").onclick = async () => {{
      const key = keyInput.value.trim();
      if (!key) {{ status.textContent = "API key required"; status.className = "err"; return; }}
      localStorage.setItem(storageKey, key);
      const body = {{
        title: document.getElementById("title").value,
        step: document.getElementById("step").value,
        acceptance: lines("acceptance"),
        constraints: lines("constraints"),
      }};
      const res = await fetch("/api/projects/" + projectId + "/goal", {{
        method: "PUT",
        headers: {{ "Content-Type": "application/json", Authorization: "Bearer " + key }},
        body: JSON.stringify(body),
      }});
      if (!res.ok) {{
        const t = await res.text();
        status.textContent = "Save failed: " + res.status + " " + t;
        status.className = "err";
        return;
      }}
      status.textContent = "Saved. Run keel cloud pull in your repo to sync locally.";
      status.className = "ok";
    }};

    keyInput.addEventListener("change", loadGoal);
    if (keyInput.value) loadGoal();
  </script>
</body>
</html>"#,
        name = name,
        id = html_escape(&project.id),
    )))
}

async fn dashboard(Path(id): Path<String>) -> Result<Html<String>, StatusCode> {
    let project = get_by_id(&id).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let project = project.ok_or(StatusCode::NOT_FOUND)?;
    let snapshot = html_escape(&project.snapshot_md);
    let name = html_escape(&project.name);
    Ok(Html(format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>Keel — {name}</title>
  <style>
    :root {{ font-family: system-ui, sans-serif; background: #0f1419; color: #e7ecf3; }}
    body {{ max-width: 820px; margin: 0 auto; padding: 2rem 1rem; }}
    h1 {{ font-size: 1.5rem; margin-bottom: .25rem; }}
    .meta {{ color: #8b9bb4; font-size: .9rem; margin-bottom: 1.5rem; }}
    pre {{ background: #1a2332; padding: 1rem; border-radius: 8px; overflow-x: auto; white-space: pre-wrap; }}
    .card {{ background: #1a2332; border-radius: 8px; padding: 1rem; margin: 1rem 0; }}
    code {{ background: #243044; padding: .15rem .4rem; border-radius: 4px; }}
    a {{ color: #6cb6ff; }}
  </style>
</head>
<body>
  <h1>Keel — {name}</h1>
  <p class="meta">Project <code>{id}</code> · updated {updated}</p>
  <p><a href="/dashboard/{id}/edit">Edit goal</a></p>
  <div class="card">
    <h2>Connect your repo</h2>
    <pre>npm install -g @keel-agent/cli
keel cloud link --url $KEEL_URL --project {id} --key YOUR_API_KEY
keel init</pre>
  </div>
  <h2>Snapshot</h2>
  <pre>{snapshot}</pre>
</body>
</html>"#,
        name = name,
        id = html_escape(&project.id),
        updated = html_escape(&project.updated_at),
        snapshot = snapshot,
    )))
}

async fn home() -> Html<&'static str> {
    Html(include_str!("../../web/index.html"))
}

async fn trust_page() -> Html<&'static str> {
    Html(include_str!("../../web/trust.html"))
}

async fn demo_gif() -> impl IntoResponse {
    let path = std::path::Path::new("/app/web/demo.gif");
    if let Ok(bytes) = std::fs::read(path) {
        return (
            [(header::CONTENT_TYPE, "image/gif")],
            bytes,
        )
            .into_response();
    }
    (
        [(header::CONTENT_TYPE, "image/gif")],
        include_bytes!("../../web/demo.gif").as_slice(),
    )
        .into_response()
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(home))
        .route("/demo.gif", get(demo_gif))
        .route("/trust", get(trust_page))
        .route("/pricing", get(pricing_page))
        .route("/team", get(team_page))
        .route("/health", get(health))
        .route("/api/projects", post(create_project_handler))
        .route("/api/projects/{id}", get(get_project))
        .route("/api/projects/{id}/sync", post(sync_handler))
        .route("/api/projects/{id}/goal", put(update_goal_handler))
        .route("/api/billing/upgrade", post(upgrade_handler))
        .route("/api/teams/projects", get(team_projects_handler))
        .route("/dashboard/{id}", get(dashboard))
        .route("/dashboard/{id}/edit", get(dashboard_edit))
        .with_state(state)
}
