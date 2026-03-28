use std::{fs, process::Command as ProcessCommand};

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::tempdir;

use dex_sim::backend::{AnvilBackend, EvmBackend, TxRequest};

fn has_anvil() -> bool {
    ProcessCommand::new("anvil")
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

#[test]
fn e2e_jsonl_run_with_mock_backend_writes_outputs() {
    let tempdir = tempdir().expect("tempdir should be created");
    let dex_path = tempdir.path().join("dex.toml");
    let history_path = tempdir.path().join("history.toml");
    let jsonl_path = tempdir.path().join("swaps.jsonl");
    let output_dir = tempdir.path().join("out");

    fs::write(&dex_path, sample_builtin_dex_toml()).expect("dex config should be written");
    fs::write(
        &history_path,
        format!(
            r#"
[source]
kind = "jsonl"
path = "{}"

[execution]
mine_after_block = true
continue_on_revert = true
"#,
            jsonl_path.display()
        ),
    )
    .expect("history config should be written");
    fs::write(
        &jsonl_path,
        r#"{"block_number":100,"sender":"0xcccccccccccccccccccccccccccccccccccccccc","token_in":"0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","token_out":"0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","amount_in":"100","min_amount_out":"91"}
{"block_number":101,"sender":"0xcccccccccccccccccccccccccccccccccccccccc","token_in":"0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","token_out":"0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","amount_in":"200","min_amount_out":"181"}
"#,
    )
    .expect("jsonl should be written");

    let mut command = Command::cargo_bin("dex-sim").expect("binary should build");
    command
        .args([
            "--dex",
            &dex_path.display().to_string(),
            "--history",
            &history_path.display().to_string(),
            "--backend",
            "mock",
            "--output-dir",
            &output_dir.display().to_string(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("completed run"))
        .stdout(predicate::str::contains("processed_blocks=2"))
        .stdout(predicate::str::contains("total_swaps=2"));

    let raw_output =
        fs::read_to_string(output_dir.join("run.raw.jsonl")).expect("raw output should exist");
    let summary_output =
        fs::read_to_string(output_dir.join("run.summary.txt")).expect("summary should exist");

    assert_eq!(raw_output.lines().count(), 2);
    assert!(raw_output.contains("\"status\":\"success\""));
    assert!(summary_output.contains("processed_blocks=2"));
    assert!(summary_output.contains("successes=2"));
}

#[test]
fn e2e_jsonl_run_with_custom_adapter_writes_outputs() {
    let tempdir = tempdir().expect("tempdir should be created");
    let dex_path = tempdir.path().join("dex.toml");
    let history_path = tempdir.path().join("history.toml");
    let jsonl_path = tempdir.path().join("swaps.jsonl");
    let output_dir = tempdir.path().join("out");

    fs::write(
        &dex_path,
        r#"
name = "my-custom-dex"
adapter = "custom.example"

[contracts]
router = "0x3333333333333333333333333333333333333333"
"#,
    )
    .expect("dex config should be written");
    fs::write(
        &history_path,
        format!(
            r#"
[source]
kind = "jsonl"
path = "{}"

[execution]
mine_after_block = true
continue_on_revert = true
"#,
            jsonl_path.display()
        ),
    )
    .expect("history config should be written");
    fs::write(
        &jsonl_path,
        r#"{"block_number":100,"sender":"0xcccccccccccccccccccccccccccccccccccccccc","token_in":"0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","token_out":"0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","amount_in":"100","min_amount_out":"91"}
"#,
    )
    .expect("jsonl should be written");

    let mut command = Command::cargo_bin("dex-sim").expect("binary should build");
    command
        .args([
            "--dex",
            &dex_path.display().to_string(),
            "--history",
            &history_path.display().to_string(),
            "--backend",
            "mock",
            "--output-dir",
            &output_dir.display().to_string(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("completed run"))
        .stdout(predicate::str::contains("successes=1"));

    let raw_output =
        fs::read_to_string(output_dir.join("run.raw.jsonl")).expect("raw output should exist");

    assert!(raw_output.contains("\"amount_out\":\"91\""));
}

#[test]
fn e2e_jsonl_run_fails_with_clear_error_for_malformed_jsonl() {
    let tempdir = tempdir().expect("tempdir should be created");
    let dex_path = tempdir.path().join("dex.toml");
    let history_path = tempdir.path().join("history.toml");
    let jsonl_path = tempdir.path().join("swaps.jsonl");
    let output_dir = tempdir.path().join("out");

    fs::write(&dex_path, sample_builtin_dex_toml()).expect("dex config should be written");
    fs::write(
        &history_path,
        format!(
            r#"
[source]
kind = "jsonl"
path = "{}"
"#,
            jsonl_path.display()
        ),
    )
    .expect("history config should be written");
    fs::write(
        &jsonl_path,
        r#"{"block_number":100,"sender":"0xcccccccccccccccccccccccccccccccccccccccc","token_in":"0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","token_out":"0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","amount_in":"100","min_amount_out":"91"}
not-json
"#,
    )
    .expect("jsonl should be written");

    let mut command = Command::cargo_bin("dex-sim").expect("binary should build");
    command
        .args([
            "--dex",
            &dex_path.display().to_string(),
            "--history",
            &history_path.display().to_string(),
            "--output-dir",
            &output_dir.display().to_string(),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("failed to parse JSONL"))
        .stderr(predicate::str::contains("line 2"));
}

#[test]
fn e2e_jsonl_run_fails_with_clear_error_for_swap_not_supported_by_dex_when_strict() {
    // When continue_on_revert = false (default), an unknown token is a fatal error.
    let tempdir = tempdir().expect("tempdir should be created");
    let dex_path = tempdir.path().join("dex.toml");
    let history_path = tempdir.path().join("history.toml");
    let jsonl_path = tempdir.path().join("swaps.jsonl");
    let output_dir = tempdir.path().join("out");

    fs::write(&dex_path, sample_builtin_dex_toml()).expect("dex config should be written");
    fs::write(
        &history_path,
        format!(
            r#"
[source]
kind = "jsonl"
path = "{}"

[execution]
mine_after_block = true
continue_on_revert = false
"#,
            jsonl_path.display()
        ),
    )
    .expect("history config should be written");
    fs::write(
        &jsonl_path,
        r#"{"block_number":100,"sender":"0xcccccccccccccccccccccccccccccccccccccccc","token_in":"0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","token_out":"0x9999999999999999999999999999999999999999","amount_in":"100","min_amount_out":"91"}
"#,
    )
    .expect("jsonl should be written");

    let mut command = Command::cargo_bin("dex-sim").expect("binary should build");
    command
        .args([
            "--dex",
            &dex_path.display().to_string(),
            "--history",
            &history_path.display().to_string(),
            "--output-dir",
            &output_dir.display().to_string(),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("token_out"))
        .stderr(predicate::str::contains("is not listed in dex config"));
}

#[test]
fn e2e_jsonl_run_skips_unknown_token_swap_when_continue_on_revert_is_true() {
    // When continue_on_revert = true, an unknown token produces a Skipped result
    // instead of aborting the whole run.
    let tempdir = tempdir().expect("tempdir should be created");
    let dex_path = tempdir.path().join("dex.toml");
    let history_path = tempdir.path().join("history.toml");
    let jsonl_path = tempdir.path().join("swaps.jsonl");
    let output_dir = tempdir.path().join("out");

    fs::write(&dex_path, sample_builtin_dex_toml()).expect("dex config should be written");
    fs::write(
        &history_path,
        format!(
            r#"
[source]
kind = "jsonl"
path = "{}"

[execution]
mine_after_block = true
continue_on_revert = true
"#,
            jsonl_path.display()
        ),
    )
    .expect("history config should be written");
    fs::write(
        &jsonl_path,
        r#"{"block_number":100,"sender":"0xcccccccccccccccccccccccccccccccccccccccc","token_in":"0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","token_out":"0x9999999999999999999999999999999999999999","amount_in":"100","min_amount_out":"91"}
"#,
    )
    .expect("jsonl should be written");

    let mut command = Command::cargo_bin("dex-sim").expect("binary should build");
    command
        .args([
            "--dex",
            &dex_path.display().to_string(),
            "--history",
            &history_path.display().to_string(),
            "--output-dir",
            &output_dir.display().to_string(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("skipped=1"));
}

#[test]
fn e2e_rpc_range_run_with_live_anvil_source_writes_outputs() {
    if !has_anvil() {
        return;
    }

    let router = "0x1111111111111111111111111111111111111111";
    let mut source_chain = AnvilBackend::spawn().expect("anvil backend should start");

    source_chain
        .send_tx(TxRequest {
            to: router.to_string(),
            data: b"swap_exact_input|sender=0xaaaa|token_in=0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa|token_out=0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb|amount_in=100|min_amount_out=91".to_vec(),
            value: 0,
        })
        .expect("first tx should succeed");
    source_chain
        .send_tx(TxRequest {
            to: router.to_string(),
            data: b"swap_exact_input|sender=0xbbbb|token_in=0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa|token_out=0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb|amount_in=200|min_amount_out=181".to_vec(),
            value: 0,
        })
        .expect("second tx should succeed");

    let tempdir = tempdir().expect("tempdir should be created");
    let dex_path = tempdir.path().join("dex.toml");
    let history_path = tempdir.path().join("history.toml");
    let output_dir = tempdir.path().join("out");

    fs::write(&dex_path, sample_builtin_dex_toml()).expect("dex config should be written");
    fs::write(
        &history_path,
        format!(
            r#"
[source]
kind = "rpc_range"
rpc_url = "{}"
start_block = 1
end_block = 2
contract_address = "{}"
method = "swap_exact_input"

[execution]
mine_after_block = true
continue_on_revert = true
"#,
            source_chain.rpc_url(),
            router
        ),
    )
    .expect("history config should be written");

    let mut command = Command::cargo_bin("dex-sim").expect("binary should build");
    command
        .args([
            "--dex",
            &dex_path.display().to_string(),
            "--history",
            &history_path.display().to_string(),
            "--backend",
            "mock",
            "--output-dir",
            &output_dir.display().to_string(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("history_source=`rpc_range`"))
        .stdout(predicate::str::contains("processed_blocks=2"))
        .stdout(predicate::str::contains("total_swaps=2"));

    let summary_output =
        fs::read_to_string(output_dir.join("run.summary.txt")).expect("summary should exist");
    let raw_output =
        fs::read_to_string(output_dir.join("run.raw.jsonl")).expect("raw output should exist");

    assert!(summary_output.contains("processed_blocks=2"));
    assert!(summary_output.contains("successes=2"));
    assert_eq!(raw_output.lines().count(), 2);
}

#[test]
fn e2e_rpc_range_run_fails_with_clear_error_for_bad_live_payload() {
    if !has_anvil() {
        return;
    }

    let router = "0x1111111111111111111111111111111111111111";
    let mut source_chain = AnvilBackend::spawn().expect("anvil backend should start");

    source_chain
        .send_tx(TxRequest {
            to: router.to_string(),
            data: b"broken-rpc-payload".to_vec(),
            value: 0,
        })
        .expect("tx should succeed");

    let tempdir = tempdir().expect("tempdir should be created");
    let dex_path = tempdir.path().join("dex.toml");
    let history_path = tempdir.path().join("history.toml");
    let output_dir = tempdir.path().join("out");

    fs::write(&dex_path, sample_builtin_dex_toml()).expect("dex config should be written");
    fs::write(
        &history_path,
        format!(
            r#"
[source]
kind = "rpc_range"
rpc_url = "{}"
start_block = 1
end_block = 1
contract_address = "{}"
method = "swap_exact_input"
"#,
            source_chain.rpc_url(),
            router
        ),
    )
    .expect("history config should be written");

    let mut command = Command::cargo_bin("dex-sim").expect("binary should build");
    command
        .args([
            "--dex",
            &dex_path.display().to_string(),
            "--history",
            &history_path.display().to_string(),
            "--backend",
            "mock",
            "--output-dir",
            &output_dir.display().to_string(),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("failed to decode swap payload"));
}

fn sample_builtin_dex_toml() -> &'static str {
    r#"
name = "mini-v2"
adapter = "builtin.v2"

[contracts]
router = "0x1111111111111111111111111111111111111111"
factory = "0x2222222222222222222222222222222222222222"

[[tokens]]
symbol = "USDC"
address = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
decimals = 6

[[tokens]]
symbol = "WETH"
address = "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
decimals = 18
"#
}
