use anyhow::Result;
use clap::{Parser, Subcommand};
use serde_json::json;

use keel::cloud::{pull_state, push_state, save_cloud_config, CloudConfig};
use keel::hooks::Agent;
use keel::install::install;
use keel::snapshot::{render_snapshot, write_snapshot};
use keel::state::{load_state, log_event, save_state, Decision, Goal};
use keel::paths::{find_project_root, utcnow};
use keel::VERSION;

#[derive(Parser)]
#[command(name = "keel", version = VERSION, about = "Repo-local agent state for Claude Code and Codex")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize .keel and install hooks
    Init,
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

fn cmd_init() -> Result<()> {
    let root = install(None)?;
    println!("Keel v{VERSION} initialized in {}", root.join(".keel").display());
    println!("Hooks installed for Claude Code and Codex (native binary)");
    println!("Next: keel goal set \"your task\" --accept \"criterion 1\"");
    Ok(())
}

fn cmd_goal_set(
    title: &str,
    accept: Vec<String>,
    constraint: Vec<String>,
    step: Option<String>,
) -> Result<()> {
    let mut state = load_state(None)?;
    state.goal = Some(Goal {
        title: title.to_string(),
        acceptance: accept,
        constraints: constraint,
        started_at: utcnow(),
    });
    if let Some(s) = step {
        state.progress.current_step = Some(s);
    }
    save_state(&mut state, None)?;
    write_snapshot(None)?;
    sync_cloud_after_write()?;
    log_event(None, "goal_set", json!({"title": title}))?;
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
    save_cloud_config(&CloudConfig {
        url: url.trim_end_matches('/').to_string(),
        project_id: project.to_string(),
        api_key: key.to_string(),
    }, None)?;
    pull_state(None)?;
    println!("Linked to Keel Cloud project {project}");
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
