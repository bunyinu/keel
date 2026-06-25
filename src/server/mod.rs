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
    count_team_projects, create_project, create_team, get_by_api_key, get_by_id, get_team_by_id,
    get_team_by_email_and_license, get_team_by_license, link_project_to_team, list_team_projects_owned, sync_project,
    upgrade_team_to_pro, valid_upgrade_code, Project, Team,
};

#[derive(Clone)]
pub struct AppState {
    pub version: String,
    pub stripe_payment_link: String,
    /// When set, POST /api/projects requires matching `X-Keel-Create-Secret` header.
    pub create_secret: Option<String>,
}

#[derive(Deserialize)]
pub struct CreateTeamRequest {
    name: String,
    #[serde(default)]
    email: Option<String>,
}

#[derive(Deserialize)]
pub struct LoginRequest {
    email: String,
    account_key: String,
}

#[derive(Serialize)]
pub struct CreateTeamResponse {
    id: String,
    name: String,
    account_key: String,
    plan: String,
    max_projects: i32,
}

#[derive(Deserialize)]
pub struct LinkProjectRequest {
    project_id: String,
    api_key: String,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    email: Option<String>,
    plan: String,
    license: String,
    max_projects: i32,
}

#[derive(Deserialize)]
pub struct SyncRequest {
    state: Value,
    snapshot: String,
    #[serde(default)]
    config: Option<Value>,
    #[serde(default)]
    changelog: Option<String>,
    #[serde(default)]
    policy: Option<Value>,
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

async fn health(axum::extract::State(state): axum::extract::State<AppState>) -> impl IntoResponse {
    Json(json!({
        "ok": true,
        "service": "keel-cloud",
        "version": state.version,
    }))
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

async fn create_team_handler(
    axum::extract::State(state): axum::extract::State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreateTeamRequest>,
) -> Result<Response, StatusCode> {
    if !create_secret_ok(&state, &headers) {
        return Ok((
            StatusCode::FORBIDDEN,
            Json(json!({"error": "Account creation is not allowed from this client"})),
        )
            .into_response());
    }
    let name = body.name.trim();
    if name.is_empty() {
        return Ok((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Account name is required"})),
        )
            .into_response());
    }
    let team = create_team(name, body.email.as_deref())
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(CreateTeamResponse {
        id: team.id,
        name: team.name,
        account_key: team.license_key,
        plan: team.plan,
        max_projects: team.max_projects,
    })
    .into_response())
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

async fn team_projects_handler(headers: HeaderMap) -> Result<Response, StatusCode> {
    let key = match extract_bearer(&headers) {
        Some(k) => k,
        None => {
            return Ok((
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Account key required"})),
            )
                .into_response());
        }
    };
    let team = get_team_by_license(&key).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let Some(team) = team else {
        return Ok((
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "Unknown account key — sign in again or create a new account"})),
        )
            .into_response());
    };
    let projects =
        list_team_projects_owned(&team.id).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({
        "team": team_json(&team),
        "projects": projects,
    }))
    .into_response())
}

async fn link_project_handler(
    headers: HeaderMap,
    Json(body): Json<LinkProjectRequest>,
) -> Result<Response, StatusCode> {
    let team_key = extract_bearer(&headers).ok_or(StatusCode::UNAUTHORIZED)?;
    let project_id = body.project_id.trim();
    let api_key = body.api_key.trim();
    if project_id.is_empty() || api_key.is_empty() {
        return Ok((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Project ID and access key are required"})),
        )
            .into_response());
    }
    let project = match link_project_to_team(project_id, api_key, &team_key) {
        Ok(p) => p,
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("project limit") {
                return Ok((
                    StatusCode::PAYMENT_REQUIRED,
                    Json(json!({"error": msg, "upgrade_url": "/pricing"})),
                )
                    .into_response());
            }
            if msg.contains("not found") || msg.contains("invalid") || msg.contains("another account")
            {
                return Ok((StatusCode::BAD_REQUEST, Json(json!({"error": msg}))).into_response());
            }
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
    Ok(Json(json!({
        "ok": true,
        "project": {
            "id": project.id,
            "name": project.name,
            "api_key": project.api_key,
            "dashboard_url": format!("/dashboard/{}", project.id),
        }
    }))
    .into_response())
}

fn team_json(team: &Team) -> TeamView {
    TeamView {
        id: team.id.clone(),
        name: team.name.clone(),
        email: team.email.clone(),
        plan: team.plan.clone(),
        license: team.license_key.clone(),
        max_projects: team.max_projects,
    }
}

async fn login_handler(Json(body): Json<LoginRequest>) -> Result<Response, StatusCode> {
    let email = body.email.trim();
    let key = body.account_key.trim();
    if email.is_empty() || key.is_empty() {
        return Ok((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Email and account key are required"})),
        )
            .into_response());
    }
    let team = get_team_by_email_and_license(email, key).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let Some(team) = team else {
        return Ok((
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "Invalid email or account key"})),
        )
            .into_response());
    };
    Ok(Json(json!({ "team": team_json(&team) })).into_response())
}

fn parse_changelog(raw: &str) -> Vec<Value> {
    raw.lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

async fn pricing_page(axum::extract::State(state): axum::extract::State<AppState>) -> Response {
    let mut html = include_str!("../../web/pricing.html").to_string();
    let link = html_escape(&state.stripe_payment_link);
    html = html.replace(
        "window.KEEL_STRIPE_PAYMENT_LINK || stripeDefault",
        &format!("\"{link}\" || stripeDefault"),
    );
    html_no_cache(html)
}

async fn get_project(
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<Value>, StatusCode> {
    let project = auth_project(&headers, &id).await?;
    let state: Value = serde_json::from_str(&project.state_json).unwrap_or(json!({}));
    let config: Value = serde_json::from_str(&project.config_json).unwrap_or(json!({}));
    let policy: Value = serde_json::from_str(&project.policy_json).unwrap_or(json!({}));
    let changelog = parse_changelog(&project.changelog_jsonl);
    Ok(Json(json!({
        "id": project.id,
        "name": project.name,
        "state": state,
        "config": config,
        "policy": policy,
        "changelog": changelog,
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
    let config_json = serde_json::to_string(&body.config.clone().unwrap_or(json!({})))
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    let changelog_jsonl = body.changelog.clone().unwrap_or_default();
    let policy_json = serde_json::to_string(&body.policy.clone().unwrap_or(json!({})))
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    sync_project(
        &id,
        &state_json,
        &body.snapshot,
        &config_json,
        &changelog_jsonl,
        &policy_json,
    )
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
    sync_project(
        &id,
        &state_json,
        &snapshot,
        "{}",
        "",
        "{}",
    )
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let state_value: Value = serde_json::from_str(&state_json).unwrap_or(json!({}));
    Ok(Json(GoalResponse {
        snapshot,
        state: state_value,
    })
    .into_response())
}

async fn dashboard_edit(Path(id): Path<String>) -> Result<Response, StatusCode> {
    dashboard_page(include_str!("../../web/dashboard-edit.html"), &id)
}

async fn dashboard(Path(id): Path<String>) -> Result<Response, StatusCode> {
    dashboard_page(include_str!("../../web/dashboard.html"), &id)
}

fn dashboard_page(template: &str, id: &str) -> Result<Response, StatusCode> {
    if get_by_id(id).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.is_none() {
        return Err(StatusCode::NOT_FOUND);
    }
    Ok(html_no_cache(
        template.replace("__PROJECT_ID__", &html_escape(id)),
    ))
}

async fn home() -> Response {
    html_static_no_cache(include_str!("../../web/index.html"))
}

async fn account_page(axum::extract::State(state): axum::extract::State<AppState>) -> Response {
    html_no_cache(inject_create_secret(
        include_str!("../../web/account.html"),
        state.create_secret.as_deref(),
    ))
}

async fn team_redirect() -> Response {
    (StatusCode::SEE_OTHER, [(header::LOCATION, "/account")]).into_response()
}

async fn start_page(axum::extract::State(state): axum::extract::State<AppState>) -> Response {
    html_no_cache(inject_create_secret(
        include_str!("../../web/start.html"),
        state.create_secret.as_deref(),
    ))
}

async fn login_redirect(request: axum::extract::Request) -> impl IntoResponse {
    let query = request.uri().query().unwrap_or("");
    redirect_to_start(query)
}

async fn new_redirect() -> impl IntoResponse {
    redirect_to_start("")
}

fn redirect_to_start(query: &str) -> Response {
    let loc = if query.is_empty() {
        "/start".to_string()
    } else {
        format!("/start?{query}")
    };
    (
        StatusCode::SEE_OTHER,
        [(header::LOCATION, loc)],
    )
        .into_response()
}

fn inject_create_secret(html: &str, secret: Option<&str>) -> String {
    let secret = secret.map(html_escape).unwrap_or_default();
    html.replace("__KEEL_CREATE_SECRET__", &secret)
}

fn html_no_cache(body: String) -> Response {
    (
        [
            (header::CACHE_CONTROL, "no-cache, no-store, must-revalidate"),
            (header::PRAGMA, "no-cache"),
        ],
        Html(body),
    )
        .into_response()
}

fn html_static_no_cache(body: &'static str) -> Response {
    (
        [
            (header::CACHE_CONTROL, "no-cache, no-store, must-revalidate"),
            (header::PRAGMA, "no-cache"),
        ],
        Html(body),
    )
        .into_response()
}

async fn app_js() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "application/javascript; charset=utf-8")],
        include_str!("../../web/app.js"),
    )
}

async fn site_css() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        include_str!("../../web/site.css"),
    )
}

async fn trust_page() -> Response {
    html_static_no_cache(include_str!("../../web/trust.html"))
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
        .route("/start", get(start_page))
        .route("/account", get(account_page))
        .route("/login", get(login_redirect))
        .route("/new", get(new_redirect))
        .route("/site.css", get(site_css))
        .route("/app.js", get(app_js))
        .route("/demo.gif", get(demo_gif))
        .route("/trust", get(trust_page))
        .route("/pricing", get(pricing_page))
        .route("/team", get(team_redirect))
        .route("/health", get(health))
        .route("/api/teams", post(create_team_handler))
        .route("/api/auth/login", post(login_handler))
        .route("/api/projects", post(create_project_handler))
        .route("/api/projects/{id}", get(get_project))
        .route("/api/projects/{id}/sync", post(sync_handler))
        .route("/api/projects/{id}/goal", put(update_goal_handler))
        .route("/api/teams/projects/link", post(link_project_handler))
        .route("/api/teams/projects", get(team_projects_handler))
        .route("/api/billing/upgrade", post(upgrade_handler))
        .route("/dashboard/{id}", get(dashboard))
        .route("/dashboard/{id}/edit", get(dashboard_edit))
        .with_state(state)
}
