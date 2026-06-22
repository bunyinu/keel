use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::json;
use std::fs;
use tempfile::TempDir;

fn bin() -> Command {
    Command::cargo_bin("keel").unwrap()
}

fn init_git_repo(dir: &std::path::Path) {
    fs::create_dir_all(dir.join(".git")).unwrap();
}

#[test]
fn init_installs_hooks_and_keel_dir() {
    let tmp = TempDir::new().unwrap();
    init_git_repo(tmp.path());

    bin()
        .current_dir(tmp.path())
        .arg("init")
        .assert()
        .success()
        .stdout(predicate::str::contains("Keel v0."));

    assert!(tmp.path().join(".keel").is_dir());
    assert!(tmp.path().join(".claude/settings.json").exists());
    assert!(tmp.path().join(".codex/hooks.json").exists());
}

#[test]
fn goal_set_writes_snapshot() {
    let tmp = TempDir::new().unwrap();
    init_git_repo(tmp.path());
    bin().current_dir(tmp.path()).args(["init"]).assert().success();

    bin()
        .current_dir(tmp.path())
        .args([
            "goal",
            "set",
            "Add OAuth",
            "--accept",
            "tests pass",
            "--step",
            "scaffold routes",
        ])
        .assert()
        .success();

    let snap = fs::read_to_string(tmp.path().join(".keel/snapshot.md")).unwrap();
    assert!(snap.contains("Add OAuth"));
    assert!(snap.contains("scaffold routes"));
}

#[test]
fn loop_breaker_blocks_third_bash_attempt() {
    let tmp = TempDir::new().unwrap();
    init_git_repo(tmp.path());
    bin().current_dir(tmp.path()).arg("init").assert().success();

    let fail_payload = json!({
        "tool_name": "Bash",
        "tool_input": {"command": "npm test"},
        "exit_code": 1,
        "stderr": "tests failed"
    })
    .to_string();

    let mut cmd = bin();
    cmd.current_dir(tmp.path())
        .args(["hook", "post-tool-use", "--agent", "claude"])
        .write_stdin(fail_payload.clone());
    cmd.assert().success();

    let mut cmd = bin();
    cmd.current_dir(tmp.path())
        .args(["hook", "post-tool-use", "--agent", "claude"])
        .write_stdin(fail_payload);
    cmd.assert().success();

    let pre_payload = json!({
        "tool_name": "Bash",
        "tool_input": {"command": "npm test"}
    })
    .to_string();

    let mut cmd = bin();
    cmd.current_dir(tmp.path())
        .args(["hook", "pre-tool-use", "--agent", "claude"])
        .write_stdin(pre_payload);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("loop breaker"));
}

#[test]
fn session_start_injects_snapshot() {
    let tmp = TempDir::new().unwrap();
    init_git_repo(tmp.path());
    bin().current_dir(tmp.path()).arg("init").assert().success();
    bin()
        .current_dir(tmp.path())
        .args(["goal", "set", "Ship it"])
        .assert()
        .success();

    let mut cmd = bin();
    cmd.current_dir(tmp.path())
        .args(["hook", "session-start", "--agent", "claude"])
        .write_stdin("{}");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Ship it"));
}

#[test]
fn status_shows_goal() {
    let tmp = TempDir::new().unwrap();
    init_git_repo(tmp.path());
    bin().current_dir(tmp.path()).arg("init").assert().success();
    bin()
        .current_dir(tmp.path())
        .args(["goal", "set", "My task"])
        .assert()
        .success();

    bin()
        .current_dir(tmp.path())
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("My task"));
}

#[test]
fn constraint_guard_blocks_npm_install() {
    let tmp = TempDir::new().unwrap();
    init_git_repo(tmp.path());
    bin().current_dir(tmp.path()).arg("init").assert().success();
    bin()
        .current_dir(tmp.path())
        .args([
            "goal",
            "set",
            "Ship",
            "--constraint",
            "no new deps",
        ])
        .assert()
        .success();

    let payload = json!({
        "tool_name": "Bash",
        "tool_input": {"command": "npm install left-pad"}
    })
    .to_string();

    let mut cmd = bin();
    cmd.current_dir(tmp.path())
        .args(["hook", "pre-tool-use", "--agent", "claude"])
        .write_stdin(payload);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("constraint guard"));
}

#[test]
fn acceptance_gate_blocks_stop_on_failure() {
    let tmp = TempDir::new().unwrap();
    init_git_repo(tmp.path());
    bin().current_dir(tmp.path()).arg("init").assert().success();
    bin()
        .current_dir(tmp.path())
        .args(["config", "set", "--acceptance", "false"])
        .assert()
        .success();

    let mut cmd = bin();
    cmd.current_dir(tmp.path())
        .args(["hook", "stop", "--agent", "claude"])
        .write_stdin("{}");
    cmd.assert()
        .code(2)
        .stdout(predicate::str::contains("acceptance gate failed"));
}

#[test]
fn doctor_passes_after_init() {
    let tmp = TempDir::new().unwrap();
    init_git_repo(tmp.path());
    bin().current_dir(tmp.path()).arg("init").assert().success();
    bin()
        .current_dir(tmp.path())
        .args(["goal", "set", "My task"])
        .assert()
        .success();

    bin()
        .current_dir(tmp.path())
        .arg("doctor")
        .assert()
        .success()
        .stdout(predicate::str::contains(".keel initialized"));
}
