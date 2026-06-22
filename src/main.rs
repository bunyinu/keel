use anyhow::Result;
use clap::{Parser, Subcommand};

use keel::cloud::{pull_state, push_state, save_cloud_config, CloudConfig};
use keel::goal_edit::{save_goal, GoalForm};
use keel::hooks::Agent;
use keel::install::install;
use keel::snapshot::{render_snapshot, write_snapshot};
use keel::state::{load_config, load_state, save_config, save_state, Decision};
use keel::paths::{find_project_root, utcnow};
use keel::VERSION;

#[derive(Parser)]
#[command(name = "keel", version = VERSION, about = "Repo-local agent state for Claude Code, Codex, and Cursor")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize .keel and install hooks
    Init,
    /// Init + set goal in one step (recommended)
    Onboard {
        title: String,
        #[arg(long, num_args = 1..)]
        accept: Vec<String>,
        #[arg(long, num_args = 1..)]
        constraint: Vec<String>,
        #[arg(long)]
        step: Option<String>,
    },
    /// Manage active goal
    Goal {
        #[command(subcommand)]
        cmd: GoalCmd,
    },
    /// Update progress
    Progress {
        #[arg(long)]
        step: Option<String>,
        #[arg(long)]
        done: Option<String>,
        #[arg(long)]
        blocker: Option<String>,
    },
    /// Record a decision
    Decide {
        text: String,
    },
    /// Show keel status
    Status,
    /// Regenerate snapshot.md
    Snapshot {
        #[arg(long)]
        print: bool,
    },
    /// Interactive goal editor (TUI)
    Tui,
    /// Update Keel to the latest release (npm)
    Update,
    /// Diagnose installation, hooks, and project setup
    Doctor,
    /// CI / workflow gate: goal present + acceptance command (if enabled)
    Check {
        /// Skip active-goal requirement
        #[arg(long)]
        no_require_goal: bool,
        /// Verify Keel Cloud link responds
        #[arg(long)]
        cloud: bool,
    },
    /// Keel configuration
    Config {
        #[command(subcommand)]
        cmd: ConfigCmd,
    },
    /// Cloud sync (Keel hosted)
    Cloud {
        #[command(subcommand)]
        cmd: CloudCmd,
    },
    /// Internal: lifecycle hook entrypoint
    #[command(hide = true)]
    Hook {
        event: String,
        #[arg(long)]
        agent: String,
    },
}

#[derive(Subcommand)]
enum GoalCmd {
    /// Set the active goal
    Set {
        title: String,
        #[arg(long, num_args = 1..)]
        accept: Vec<String>,
        #[arg(long, num_args = 1..)]
        constraint: Vec<String>,
        #[arg(long)]
        step: Option<String>,
    },
    /// Show active goal as JSON
    Show,
}

#[derive(Subcommand)]
enum ConfigCmd {
    /// Show config.json
    Show,
    /// Set configuration values
    Set {
        /// Shell command for acceptance gate (use "off" to disable)
        #[arg(long)]
        acceptance: Option<String>,
    },
}

#[derive(Subcommand)]
enum CloudCmd {
    /// Link this repo to Keel Cloud
    Link {
        #[arg(long)]
        url: String,
        #[arg(long)]
        project: String,
        #[arg(long)]
        key: String,
    },
    /// Push local state to cloud
    Push,
    /// Pull state from cloud
    Pull,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Init => cmd_init(),
        Commands::Onboard {
            title,
            accept,
            constraint,
            step,
        } => keel::onboard::run_onboard(&title, accept, constraint, step, None),
        Commands::Goal { cmd } => match cmd {
            GoalCmd::Set {
                title,
                accept,
                constraint,
                step,
            } => cmd_goal_set(&title, accept, constraint, step),
            GoalCmd::Show => cmd_goal_show(),
        },
        Commands::Progress { step, done, blocker } => cmd_progress(step, done, blocker),
        Commands::Decide { text } => cmd_decide(&text),
        Commands::Status => cmd_status(),
        Commands::Snapshot { print } => cmd_snapshot(print),
        Commands::Tui => keel::tui::run_tui(),
        Commands::Update => cmd_update(),
        Commands::Doctor => cmd_doctor(),
        Commands::Check {
            no_require_goal,
            cloud,
        } => cmd_check(no_require_goal, cloud),
        Commands::Config { cmd } => match cmd {
            ConfigCmd::Show => cmd_config_show(),
            ConfigCmd::Set { acceptance } => cmd_config_set(acceptance),
        },
        Commands::Cloud { cmd } => match cmd {
            CloudCmd::Link { url, project, key } => cmd_cloud_link(&url, &project, &key),
            CloudCmd::Push => cmd_cloud_push(),
            CloudCmd::Pull => cmd_cloud_pull(),
        },
        Commands::Hook { event, agent } => {
            let agent = Agent::parse(&agent).ok_or_else(|| anyhow::anyhow!("invalid agent"))?;
            keel::hooks::run_hook(&event, agent)?;
            Ok(())
        }
    }
}

fn cmd_update() -> Result<()> {
    if std::env::var("KEEL_MANAGED_BY_NPM").is_ok() {
        anyhow::bail!("update is handled by the npm shim; re-run: keel update");
    }

    let npm = std::process::Command::new("npm")
        .arg("--version")
        .output()
        .ok()
        .filter(|o| o.status.success());

    if npm.is_some() {
        println!("Updating Keel via npm (@keel-agent/cli@latest)...");
        let status = std::process::Command::new("npm")
            .args(["install", "-g", "@keel-agent/cli@latest"])
            .status()?;
        if !status.success() {
            anyhow::bail!("npm install failed");
        }
        println!("Done. Run: keel --version");
        println!("If the version is stale, open a new terminal or run: hash -r");
        return Ok(());
    }

    anyhow::bail!(
        "Install and update Keel with npm (the standard method):\n\n  npm install -g @keel-agent/cli@latest\n\nRequires Node.js 18+."
    );
}

fn cmd_doctor() -> Result<()> {
    let checks = keel::doctor::run_doctor()?;
    let ok = keel::doctor::print_report(&checks);
    if ok {
        println!("\nKeel doctor: all critical checks passed.");
    } else {
        println!("\nKeel doctor: fix the items marked ✗ above.");
        std::process::exit(1);
    }
    Ok(())
}

fn cmd_check(no_require_goal: bool, cloud: bool) -> Result<()> {
    keel::check::run_check(
        keel::check::CheckOptions {
            require_goal: !no_require_goal,
            verify_cloud: cloud,
        },
        None,
    )?;
    println!("Keel check: passed");
    Ok(())
}

fn cmd_config_show() -> Result<()> {
    let config = load_config(None)?;
    println!("{}", serde_json::to_string_pretty(&config)?);
    Ok(())
}

fn cmd_config_set(acceptance: Option<String>) -> Result<()> {
    let Some(val) = acceptance else {
        anyhow::bail!("usage: keel config set --acceptance \"npm test\"  (or --acceptance off)");
    };
    let mut config = load_config(None)?;
    if val.eq_ignore_ascii_case("off") {
        config.acceptance_gate.enabled = false;
        config.acceptance_gate.command.clear();
        save_config(&config, None)?;
        println!("Acceptance gate disabled");
        return Ok(());
    }
    config.acceptance_gate.enabled = true;
    config.acceptance_gate.command = val.clone();
    save_config(&config, None)?;
    println!("Acceptance gate enabled: {val}");
    println!("Runs on agent Stop hook before session ends.");
    Ok(())
}

fn cmd_init() -> Result<()> {
    let root = install(None)?;
    println!("Keel v{VERSION} initialized in {}", root.join(".keel").display());
    println!("Hooks installed for Claude Code, Codex, and Cursor");
    println!("Next: keel onboard \"your task\" --accept \"criterion 1\"");
    println!("     or: keel goal set \"...\" / keel tui");
    Ok(())
}

fn cmd_goal_set(
    title: &str,
    accept: Vec<String>,
    constraint: Vec<String>,
    step: Option<String>,
) -> Result<()> {
    let form = GoalForm {
        title: title.to_string(),
        step: step.unwrap_or_default(),
        acceptance: accept,
        constraints: constraint,
    };
    save_goal(&form, None, "cli")?;
    println!("Goal set: {title}");
    Ok(())
}

fn cmd_goal_show() -> Result<()> {
    let state = load_state(None)?;
    match state.goal {
        Some(goal) => println!("{}", serde_json::to_string_pretty(&goal)?),
        None => println!("No active goal. Run: keel goal set \"...\""),
    }
    Ok(())
}

fn cmd_progress(
    step: Option<String>,
    done: Option<String>,
    blocker: Option<String>,
) -> Result<()> {
    let mut state = load_state(None)?;
    if let Some(s) = step {
        state.progress.current_step = Some(s.clone());
        println!("Current step: {s}");
    }
    if let Some(d) = done {
        state.progress.completed.push(d.clone());
        println!("Marked done: {d}");
    }
    if let Some(b) = blocker {
        state.progress.blockers.push(b.clone());
        println!("Blocker: {b}");
    }
    save_state(&mut state, None)?;
    write_snapshot(None)?;
    sync_cloud_after_write()?;
    Ok(())
}

fn cmd_decide(text: &str) -> Result<()> {
    let mut state = load_state(None)?;
    state.decisions.push(Decision {
        at: utcnow(),
        text: text.to_string(),
    });
    save_state(&mut state, None)?;
    write_snapshot(None)?;
    sync_cloud_after_write()?;
    println!("Recorded decision: {text}");
    Ok(())
}

fn cmd_status() -> Result<()> {
    let root = find_project_root(None);
    let state = load_state(None)?;
    let goal = state.goal.as_ref().map(|g| g.title.as_str()).unwrap_or("(none)");
    let step = state
        .progress
        .current_step
        .as_deref()
        .unwrap_or("(none)");
    println!("Project: {}", root.display());
    println!("Goal: {goal}");
    println!("Step: {step}");
    println!(
        "Compactions: {} · Sessions: {}",
        state.compactions, state.sessions
    );
    println!(
        "Last agent: {}",
        state.last_agent.as_deref().unwrap_or("unknown")
    );
    println!("Snapshot: {}", root.join(".keel/snapshot.md").display());
    Ok(())
}

fn cmd_snapshot(print: bool) -> Result<()> {
    if print {
        print!("{}", render_snapshot(None)?);
    } else {
        let path = write_snapshot(None)?;
        sync_cloud_after_write()?;
        println!("Wrote {}", path.display());
    }
    Ok(())
}

fn sync_cloud_after_write() -> Result<()> {
    push_state(None)
}

fn cmd_cloud_link(url: &str, project: &str, key: &str) -> Result<()> {
    let cwd = std::env::current_dir()?;
    if !cwd.join(".git").exists() && !cwd.join(".keel").exists() {
        eprintln!("Tip: run `cd your-project` before `keel cloud link` so state stays in the repo.");
    }
    save_cloud_config(&CloudConfig {
        url: url.trim_end_matches('/').to_string(),
        project_id: project.to_string(),
        api_key: key.to_string(),
    }, None)?;
    let local_before = load_state(None)?;
    pull_state(None)?;
    let pulled = load_state(None)?;
    // New cloud projects start with `{}` — do not wipe an existing local goal.
    if local_before.goal.is_some() && pulled.goal.is_none() {
        let mut restored = local_before;
        save_state(&mut restored, None)?;
        write_snapshot(None)?;
        push_state(None)?;
        println!("Linked to Keel Cloud project {project}");
        println!("Uploaded local state (cloud project was empty).");
    } else {
        println!("Linked to Keel Cloud project {project}");
        println!("Pulled state from cloud.");
    }
    println!("URL: {}", url.trim_end_matches('/'));
    Ok(())
}

fn cmd_cloud_push() -> Result<()> {
    write_snapshot(None)?;
    push_state(None)?;
    println!("Pushed to Keel Cloud");
    Ok(())
}

fn cmd_cloud_pull() -> Result<()> {
    pull_state(None)?;
    println!("Pulled from Keel Cloud");
    Ok(())
}
