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
    /// When set, POST /api/projects requires matching `X-Keel-Create-Secret` header.
    pub create_secret: Option<String>,
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

fn create_secret_ok(state: &AppState, headers: &HeaderMap) -> bool {
    match &state.create_secret {
        None => true,
        Some(expected) => headers
            .get("x-keel-create-secret")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|got| got == expected),
    }
}

async fn create_project_handler(
    axum::extract::State(state): axum::extract::State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreateProjectRequest>,
) -> Result<Response, StatusCode> {
    if !create_secret_ok(&state, &headers) {
        return Ok((
            StatusCode::FORBIDDEN,
            Json(json!({"error": "Project creation is not allowed from this client"})),
        )
            .into_response());
    }
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
    dashboard_page(include_str!("../../web/dashboard-edit.html"), &id)
}

async fn dashboard(Path(id): Path<String>) -> Result<Html<String>, StatusCode> {
    dashboard_page(include_str!("../../web/dashboard.html"), &id)
}

fn dashboard_page(template: &str, id: &str) -> Result<Html<String>, StatusCode> {
    if get_by_id(id).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.is_none() {
        return Err(StatusCode::NOT_FOUND);
    }
    Ok(Html(
        template.replace("__PROJECT_ID__", &html_escape(id)),
    ))
}

async fn home() -> Html<&'static str> {
    Html(include_str!("../../web/index.html"))
}

async fn login_page(axum::extract::State(state): axum::extract::State<AppState>) -> Html<String> {
    Html(inject_create_secret(
        include_str!("../../web/login.html"),
        state.create_secret.as_deref(),
    ))
}

async fn new_redirect() -> impl IntoResponse {
    (
        StatusCode::SEE_OTHER,
        [(header::LOCATION, "/login?tab=create")],
    )
}

fn inject_create_secret(html: &str, secret: Option<&str>) -> String {
    let secret = secret.map(html_escape).unwrap_or_default();
    html.replace("__KEEL_CREATE_SECRET__", &secret)
}

async fn site_css() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        include_str!("../../web/site.css"),
    )
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
        .route("/login", get(login_page))
        .route("/new", get(new_redirect))
        .route("/site.css", get(site_css))
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
