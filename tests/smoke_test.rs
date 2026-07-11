/// Smoke tests to verify the binary runs without panicking
use std::process::Command;

const SUBCOMMANDS: &[&str] = &[
    "bonsai",
    "matrix",
    "life",
    "plasma",
    "fire",
    "rain",
    "waves",
    "cube",
    "hypercube",
    "pipes",
    "donut",
    "globe",
    "hex",
    "keyboard",
    "dygma",
    "invaders",
    "audio",
    "lissajous",
    "fractal",
    "clock",
    "sunlight",
    "pong",
    "cpu",
    "mem",
    "disk",
    "io",
    "net",
    "gpu",
    "ps",
    "docker",
    "weather",
    "pomodoro",
    "fah",
    "tui-cover",
    "tui-control",
    "claude-tokens",
    "codex-tokens",
];

fn termart() -> Command {
    Command::new(env!("CARGO_BIN_EXE_termart"))
}

#[test]
fn binary_shows_help() {
    let output = termart()
        .arg("--help")
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
    let output = termart()
        .arg("--version")
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
    let output = termart()
        .arg("nonexistent-command")
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

#[test]
fn every_subcommand_has_consistent_help_output() {
    for command in SUBCOMMANDS {
        let output = termart()
            .args([command, "--help"])
            .output()
            .unwrap_or_else(|error| panic!("Failed to run {command} --help: {error}"));

        assert!(
            output.status.success(),
            "{command} --help failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains(&format!("Usage: termart {command}")),
            "{command} help should contain its standard Usage line"
        );
        assert!(
            stdout.contains("Options:"),
            "{command} help should contain a standard Options section"
        );
        assert!(
            stdout.contains("-h, --help"),
            "{command} help should expose the standard help option"
        );
    }
}
