pub mod db;

use axum::{
    extract::Path,
    http::{header, HeaderMap, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use self::db::{create_project, get_by_api_key, get_by_id, sync_project, Project};

#[derive(Clone)]
pub struct AppState {
    pub version: String,
}

#[derive(Deserialize)]
pub struct CreateProjectRequest {
    name: String,
}

#[derive(Serialize)]
pub struct CreateProjectResponse {
    id: String,
    name: String,
    api_key: String,
    dashboard_url: String,
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

async fn create_project_handler(Json(body): Json<CreateProjectRequest>) -> Result<Response, StatusCode> {
    let name = body.name.trim();
    if name.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
    let project = create_project(name).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(CreateProjectResponse {
        id: project.id.clone(),
        name: project.name,
        api_key: project.api_key,
        dashboard_url: format!("/dashboard/{}", project.id),
    })
    .into_response())
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

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(home))
        .route("/health", get(health))
        .route("/api/projects", post(create_project_handler))
        .route("/api/projects/{id}", get(get_project))
        .route("/api/projects/{id}/sync", post(sync_handler))
        .route("/dashboard/{id}", get(dashboard))
        .with_state(state)
}
