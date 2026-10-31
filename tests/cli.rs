//! Smoke tests for the `framecensus` binary.

use assert_cmd::Command;

#[test]
fn help_prints_usage() {
    let assert = Command::cargo_bin("framecensus")
        .expect("binary should build")
        .arg("--help")
        .assert()
        .success();
    let output = assert.get_output();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Usage: framecensus"),
        "help output did not contain usage: {stdout}"
    );
}

#[test]
fn no_arguments_prints_usage() {
    let assert = Command::cargo_bin("framecensus")
        .expect("binary should build")
        .assert()
        .code(2);
    let output = assert.get_output();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Usage: framecensus"),
        "usage output did not appear on stderr: {stderr}"
    );
}
