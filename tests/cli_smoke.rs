use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn cli_help_lists_required_arguments() {
    let mut command = Command::cargo_bin("dex-sim").expect("binary should build");

    command
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("--dex"))
        .stdout(predicate::str::contains("--history"))
        .stdout(predicate::str::contains("--backend"))
        .stdout(predicate::str::contains("--output-dir"));
}
