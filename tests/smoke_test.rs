/// Smoke tests to verify the binary runs without panicking
use std::process::Command;

#[test]
fn binary_shows_help() {
    let output = Command::new("cargo")
        .args(["run", "--", "--help"])
        .output()
        .expect("Failed to execute cargo run");

    assert!(
        output.status.success(),
        "Binary failed to run --help: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("termart"),
        "Help output should mention termart"
    );
}

#[test]
fn binary_shows_version() {
    let output = Command::new("cargo")
        .args(["run", "--", "--version"])
        .output()
        .expect("Failed to execute cargo run");

    assert!(
        output.status.success(),
        "Binary failed to run --version: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn invalid_subcommand_fails_gracefully() {
    let output = Command::new("cargo")
        .args(["run", "--", "nonexistent-command"])
        .output()
        .expect("Failed to execute cargo run");

    // Should fail with error, not panic
    assert!(
        !output.status.success(),
        "Invalid subcommand should return error status"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    // Should show helpful error, not a panic backtrace
    assert!(
        !stderr.contains("panicked at"),
        "Invalid subcommand should not cause panic"
    );
}
