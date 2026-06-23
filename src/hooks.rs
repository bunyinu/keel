use anyhow::Result;
use serde_json::{json, Value};
use std::io::{self, Read};
use std::process::exit;

use crate::acceptance::run_acceptance_gate;
use crate::constraints::{check_pre_tool_constraints, record_violation};
use crate::loop_breaker::{check_pre_tool, record_tool_result};
use crate::snapshot::{render_snapshot, write_snapshot};
use crate::state::{load_state, log_event, save_state, KeelState};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Agent {
    Claude,
    Codex,
    Cursor,
}

impl Agent {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "claude" => Some(Self::Claude),
            "codex" => Some(Self::Codex),
            "cursor" => Some(Self::Cursor),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::Cursor => "cursor",
        }
    }
}

/// Map Cursor tool names to the canonical names used by loop breaker / constraints.
pub fn normalize_tool_name(tool: &str) -> &str {
    match tool {
        "Shell" => "Bash",
        "TabWrite" | "TabEdit" => "Write",
        _ => tool,
    }
}

pub fn read_stdin_json() -> Result<Value> {
    let mut raw = String::new();
    io::stdin().read_to_string(&mut raw)?;
    if raw.trim().is_empty() {
        return Ok(json!({}));
    }
    Ok(serde_json::from_str(&raw)?)
}

fn emit_claude_block(reason: &str) -> ! {
    println!("{}", json!({"decision": "block", "reason": reason}));
    exit(0);
}

fn emit_codex_block(reason: &str) -> ! {
    println!(
        "{}",
        json!({
            "hookSpecificOutput": {
                "hookEventName": "PreToolUse",
                "permissionDecision": "deny",
                "permissionDecisionReason": reason,
            }
        })
    );
    exit(0);
}

fn emit_cursor_block(reason: &str) -> ! {
    println!(
        "{}",
        json!({
            "permission": "deny",
            "user_message": reason,
            "agent_message": reason,
        })
    );
    exit(2);
}

fn emit_cursor_context(text: &str) -> ! {
    println!("{}", json!({"additional_context": text}));
    exit(0);
}

fn emit_codex_context(event: &str, text: &str) -> ! {
    println!(
        "{}",
        json!({
            "hookSpecificOutput": {
                "hookEventName": event,
                "additionalContext": text,
            }
        })
    );
    exit(0);
}

fn snapshot_text() -> Result<String> {
    write_snapshot(None)?;
    render_snapshot(None)
}

fn bump(state: &mut KeelState, field: &str) {
    match field {
        "compactions" => state.compactions += 1,
        "sessions" => state.sessions += 1,
        _ => {}
    }
}

pub fn run_hook(event: &str, agent: Agent) -> Result<()> {
    match event {
        "pre-compact" => handle_pre_compact(agent),
        "post-compact" => handle_post_compact(agent),
        "session-start" => handle_session_start(agent),
        "pre-tool-use" => handle_pre_tool_use(agent),
        "post-tool-use" => handle_post_tool_use(agent),
        "stop" => handle_stop(agent),
        "user-prompt-submit" => handle_user_prompt_submit(agent),
        other => anyhow::bail!("unknown hook: {other}"),
    }
}

fn sync_cloud_quiet() {
    let _ = crate::cloud::push_state(None);
}

fn handle_pre_compact(agent: Agent) -> Result<()> {
    let payload = read_stdin_json()?;
    let mut state = load_state(None)?;
    bump(&mut state, "compactions");
    state.last_agent = Some(agent.as_str().into());
    save_state(&mut state, None)?;
    write_snapshot(None)?;
    sync_cloud_quiet();
    log_event(
        None,
        "pre_compact",
        json!({"agent": agent.as_str(), "trigger": payload.get("trigger")}),
    )?;

    if agent == Agent::Claude {
        let ctx = snapshot_text()?;
        // Exit 0 + systemMessage: preserve task state through compaction (do NOT exit 2 — that blocks compact).
        println!(
            "{}",
            json!({
                "systemMessage": format!(
                    "Keel task state to preserve through compaction:\n\n{ctx}"
                ),
            })
        );
    } else if agent == Agent::Cursor {
        let ctx = snapshot_text()?;
        println!(
            "{}",
            json!({
                "agent_message": format!(
                    "Keel task state to preserve through compaction:\n\n{ctx}"
                ),
            })
        );
    }
    Ok(())
}

fn handle_post_compact(agent: Agent) -> Result<()> {
    let _ = read_stdin_json()?;
    write_snapshot(None)?;
    log_event(None, "post_compact", json!({"agent": agent.as_str()}))?;
    let ctx = snapshot_text()?;
    match agent {
        Agent::Codex => emit_codex_context("PostCompact", &ctx),
        Agent::Cursor => emit_cursor_context(&ctx),
        Agent::Claude => print!("{ctx}"),
    }
    Ok(())
}

fn handle_session_start(agent: Agent) -> Result<()> {
    let payload = read_stdin_json()?;
    let _ = crate::cloud::pull_state(None);
    let mut state = load_state(None)?;
    bump(&mut state, "sessions");
    state.last_agent = Some(agent.as_str().into());
    save_state(&mut state, None)?;
    write_snapshot(None)?;
    sync_cloud_quiet();
    let source = payload
        .get("source")
        .or(payload.get("session_type"))
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    log_event(
        None,
        "session_start",
        json!({"agent": agent.as_str(), "source": source}),
    )?;
    let ctx = snapshot_text()?;
    match agent {
        Agent::Codex => emit_codex_context("SessionStart", &ctx),
        Agent::Cursor => emit_cursor_context(&ctx),
        Agent::Claude => print!("{ctx}"),
    }
    Ok(())
}

fn cursor_tool_input(payload: &Value) -> Value {
    if let Some(input) = payload.get("tool_input").or(payload.get("input")) {
        return input.clone();
    }
    // Cursor beforeShellExecution-style payloads
    if let Some(cmd) = payload.get("command").and_then(|c| c.as_str()) {
        return json!({"command": cmd});
    }
    json!({})
}

fn handle_pre_tool_use(agent: Agent) -> Result<()> {
    let payload = read_stdin_json()?;
    let raw_tool = payload
        .get("tool_name")
        .or(payload.get("tool"))
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let tool = normalize_tool_name(raw_tool);
    let tool_input = cursor_tool_input(&payload);

    let (block, reason) = check_pre_tool(None, agent.as_str(), tool, &tool_input)?;
    if block {
        match agent {
            Agent::Codex => emit_codex_block(&reason),
            Agent::Cursor => emit_cursor_block(&reason),
            Agent::Claude => emit_claude_block(&reason),
        }
    }

    if let Some(reason) = crate::policy::hook_block_reason(None)? {
        match agent {
            Agent::Codex => emit_codex_block(&reason),
            Agent::Cursor => emit_cursor_block(&reason),
            Agent::Claude => emit_claude_block(&reason),
        }
    }

    let (block, reason) = check_pre_tool_constraints(None, tool, &tool_input)?;
    if block {
        let _ = record_violation(None, &reason);
        match agent {
            Agent::Codex => emit_codex_block(&reason),
            Agent::Cursor => emit_cursor_block(&reason),
            Agent::Claude => emit_claude_block(&reason),
        }
    }
    Ok(())
}

fn handle_post_tool_use(agent: Agent) -> Result<()> {
    let payload = read_stdin_json()?;
    let raw_tool = payload
        .get("tool_name")
        .or(payload.get("tool"))
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let tool = normalize_tool_name(raw_tool);
    let tool_input = cursor_tool_input(&payload);

    let (ok, _, _) = crate::loop_breaker::detect_tool_failure(&payload);
    record_tool_result(None, agent.as_str(), tool, &tool_input, &payload)?;
    if !ok {
        write_snapshot(None)?;
        sync_cloud_quiet();
    }
    Ok(())
}

fn handle_stop(agent: Agent) -> Result<()> {
    let (ok, reason) = run_acceptance_gate(None)?;
    if ok {
        return Ok(());
    }
    match agent {
        Agent::Codex => {
            println!(
                "{}",
                json!({
                    "continue": false,
                    "systemMessage": reason,
                })
            );
            exit(0);
        }
        Agent::Cursor => {
            println!(
                "{}",
                json!({
                    "followup_message": reason,
                    "agent_message": reason,
                })
            );
            exit(2);
        }
        Agent::Claude => {
            println!(
                "{}",
                json!({
                    "continue": false,
                    "systemMessage": reason,
                })
            );
            exit(2);
        }
    }
}

fn handle_user_prompt_submit(agent: Agent) -> Result<()> {
    let payload = read_stdin_json()?;
    let prompt = payload
        .get("prompt")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let short: String = prompt.chars().take(500).collect();
    log_event(
        None,
        "user_prompt",
        json!({"agent": agent.as_str(), "prompt": short}),
    )?;
    let ctx = "Keel: If you lost context, read `.keel/snapshot.md` before acting. \
               Do not repeat approaches listed under Do NOT retry.";
    match agent {
        Agent::Codex => emit_codex_context("UserPromptSubmit", ctx),
        Agent::Cursor => emit_cursor_context(ctx),
        Agent::Claude => println!("{ctx}"),
    }
    Ok(())
}
