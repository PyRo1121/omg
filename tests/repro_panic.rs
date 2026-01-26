#![allow(clippy::uninlined_format_args)]

use std::process::{Command, Stdio};

#[test]
fn repro_node_latest_panic() {
    let output = Command::new(env!("CARGO_BIN_EXE_omg"))
        .args(["use", "node", "latest"])
        .env("OMG_TEST_MODE", "1")
        .env("OMG_DISABLE_DAEMON", "1")
        .env("RUST_BACKTRACE", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("Failed to execute omg");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    println!("STDOUT:\n{}", stdout);
    println!("STDERR:\n{}", stderr);

    if !output.status.success() {
        if stderr.contains("panicked at") {
            panic!("Panic detected in omg binary!");
        } else {
            panic!("Command failed without panic: {}", output.status);
        }
    }
}
