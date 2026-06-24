use anyhow::Context;
use keel::server::db::init_db;
use keel::server::{router, AppState};
use keel::VERSION;
use std::net::SocketAddr;
use std::path::PathBuf;
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("keel_server=info".parse()?))
        .init();

    let db_path = std::env::var("KEEL_DB_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/data/keel.db"));

    init_db(&db_path).with_context(|| format!("init database at {}", db_path.display()))?;
    tracing::info!("database: {}", db_path.display());

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    let app = router(AppState {
        version: VERSION.to_string(),
        stripe_payment_link: std::env::var("KEEL_STRIPE_PAYMENT_LINK").unwrap_or_else(|_| {
            "https://buy.stripe.com/test_keel_pro_placeholder".to_string()
        }),
        create_secret: std::env::var("KEEL_CREATE_SECRET").ok().filter(|s| !s.is_empty()),
    })
    .layer(CorsLayer::permissive())
    .layer(TraceLayer::new_for_http());

    let listener = TcpListener::bind(addr).await?;
    tracing::info!("keel-server v{VERSION} listening on {addr}");
    axum::serve(listener, app).await?;
    Ok(())
}
