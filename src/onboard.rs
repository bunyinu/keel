use anyhow::{bail, Result};
use std::path::Path;

use crate::goal_edit::{save_goal, GoalForm};
use crate::install::install;

/// `keel onboard` — init + set goal in one step (avoids empty `.keel`).
pub fn run_onboard(
    title: &str,
    accept: Vec<String>,
    constraint: Vec<String>,
    step: Option<String>,
    root: Option<&Path>,
) -> Result<()> {
    let title = title.trim();
    if title.is_empty() {
        bail!(
            "Goal title is required.\n\nExample:\n  \
             keel onboard \"Ship auth\" --accept \"tests pass\" --step \"scaffold routes\""
        );
    }

    let project = install(root)?;
    let form = GoalForm {
        title: title.to_string(),
        step: step.unwrap_or_default(),
        acceptance: accept,
        constraints: constraint,
    };
    save_goal(&form, Some(&project), "onboard")?;

    println!("Keel onboard complete in {}", project.display());
    println!("Goal: {title}");
    println!("Hooks: .claude/ · .codex/ · .cursor/");
    println!("State: {}/snapshot.md", project.join(".keel").display());
    println!("\nNext: open Claude Code, Codex, or Cursor in this repo — goal survives compaction.");
    Ok(())
}
