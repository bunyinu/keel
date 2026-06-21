pub mod cloud;
pub mod hooks;
pub mod install;
pub mod loop_breaker;
pub mod paths;
pub mod server;
pub mod snapshot;
pub mod state;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const KEEL_DIR: &str = ".keel";
