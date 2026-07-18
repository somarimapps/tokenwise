use assert_cmd::Command;
use predicates::prelude::*;

/// T-023: `--help` exits 0 and contains the binary name.
#[test]
fn help_exits_zero() {
    let mut cmd = Command::cargo_bin("tokenwise").unwrap();
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("tokenwise"));
}

/// T-023: `--version` exits 0 and prints a version string.
#[test]
fn version_exits_zero() {
    let mut cmd = Command::cargo_bin("tokenwise").unwrap();
    cmd.arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("tokenwise"));
}

/// T-023: Unknown subcommand exits non-zero (clap returns exit code 2 for usage errors).
#[test]
fn unknown_subcommand_exits_nonzero() {
    let mut cmd = Command::cargo_bin("tokenwise").unwrap();
    cmd.arg("unknown-subcommand-xyz")
        .assert()
        .failure();
}

/// T-031: `install` subcommand runs (PR2 — exits 0 when all components present,
/// exits 1 when some components fail to install).
#[test]
fn install_runs_without_panic() {
    let mut cmd = Command::cargo_bin("tokenwise").unwrap();
    let output = cmd.arg("install").output().unwrap();
    let code = output.status.code().unwrap_or(-1);
    // 0 = all installed/skipped, 1 = some failed — both are valid outcomes.
    // Anything else (e.g. panic/signal) is a bug.
    assert!(
        code == 0 || code == 1,
        "install must exit with 0 or 1, got {code}"
    );
}

/// T-029: `stats` subcommand exits 0 and prints a table (all values may be `—`
/// on a machine without the full stack installed).
#[test]
fn stats_exits_success() {
    let mut cmd = Command::cargo_bin("tokenwise").unwrap();
    cmd.arg("stats")
        .assert()
        .success()
        .stdout(predicate::str::contains("Headroom savings"));
}

/// T-023: `connect` requires a target argument — exits nonzero without it.
#[test]
fn connect_requires_target_arg() {
    let mut cmd = Command::cargo_bin("tokenwise").unwrap();
    cmd.arg("connect")
        .assert()
        .failure();
}

/// T-018: `connect claude` runs without panicking (PR2 implemented).
/// May exit 0 (connected) or 1 (env error, e.g. home dir inaccessible).
/// Must NOT print "not yet implemented".
#[test]
fn connect_claude_runs_without_panic() {
    let mut cmd = Command::cargo_bin("tokenwise").unwrap();
    let output = cmd.args(["connect", "claude"]).output().unwrap();
    let code = output.status.code().unwrap_or(-1);
    assert!(
        code == 0 || code == 1,
        "connect claude must exit with 0 or 1, got {code}"
    );
    // Must not still be a PR1 stub.
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("not yet implemented"),
        "connect claude must not output the PR1 stub message: {stderr}"
    );
}

/// T-023: `doctor` runs and produces layered output.
/// This test only verifies it doesn't crash and produces structured output.
/// On CI most layers will FAIL/WARN (tools not installed) — that's expected.
#[test]
fn doctor_produces_layered_output() {
    let mut cmd = Command::cargo_bin("tokenwise").unwrap();
    let output = cmd.arg("doctor").output().unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Doctor must emit at least one bracketed status line
    assert!(
        stdout.contains("[PASS]") || stdout.contains("[WARN]") || stdout.contains("[FAIL]"),
        "Doctor output must contain at least one [PASS]/[WARN]/[FAIL] line:\n{}",
        stdout
    );
}

/// T-023: `sync` runs against a temp-less environment — should succeed or fail gracefully.
#[test]
fn sync_does_not_panic() {
    let mut cmd = Command::cargo_bin("tokenwise").unwrap();
    // We don't assert success here because settings.json may not exist in test env.
    // We only assert the process exits cleanly (not a panic/segfault).
    let output = cmd.arg("sync").output().unwrap();
    let code = output.status.code().unwrap_or(-1);
    // exit code 0 or 1 are both acceptable; anything else suggests a crash
    assert!(
        code == 0 || code == 1,
        "sync must exit with 0 or 1, got {}",
        code
    );
}

/// T-023: `doctor --help` exits 0.
#[test]
fn doctor_help_exits_zero() {
    let mut cmd = Command::cargo_bin("tokenwise").unwrap();
    cmd.args(["doctor", "--help"])
        .assert()
        .success();
}
