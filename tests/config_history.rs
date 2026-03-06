use std::fs;

use dex_sim::config::load_history_config;
use tempfile::tempdir;

#[test]
fn parses_jsonl_history_config() {
    let tempdir = tempdir().expect("tempdir should be created");
    let path = tempdir.path().join("history.toml");

    fs::write(
        &path,
        r#"
name = "sample-jsonl"

[source]
kind = "jsonl"
path = "../datasets/swaps/sample.jsonl"

[execution]
mine_after_block = true
continue_on_revert = false
"#,
    )
    .expect("config should be written");

    let config = load_history_config(&path).expect("config should parse");

    assert_eq!(config.name.as_deref(), Some("sample-jsonl"));
    assert_eq!(config.source.kind(), "jsonl");
    assert!(config.execution.mine_after_block);
    match config.source {
        dex_sim::config::HistorySourceConfig::Jsonl { path } => {
            assert_eq!(path, tempdir.path().join("../datasets/swaps/sample.jsonl"));
        }
        _ => panic!("expected jsonl source"),
    }
}

#[test]
fn parses_rpc_range_history_config() {
    let tempdir = tempdir().expect("tempdir should be created");
    let path = tempdir.path().join("history.toml");

    fs::write(
        &path,
        r#"
[source]
kind = "rpc_range"
rpc_url = "http://localhost:8545"
start_block = 100
end_block = 120
contract_address = "0x1111111111111111111111111111111111111111"
method = "swapExactTokensForTokens"
"#,
    )
    .expect("config should be written");

    let config = load_history_config(&path).expect("config should parse");

    assert_eq!(config.source.kind(), "rpc_range");
}

#[test]
fn rejects_invalid_rpc_range() {
    let tempdir = tempdir().expect("tempdir should be created");
    let path = tempdir.path().join("history.toml");

    fs::write(
        &path,
        r#"
[source]
kind = "rpc_range"
rpc_url = "http://localhost:8545"
start_block = 120
end_block = 100
contract_address = "0x1111111111111111111111111111111111111111"
"#,
    )
    .expect("config should be written");

    let error = load_history_config(&path).expect_err("config should fail validation");

    assert!(error.to_string().contains("start_block"));
}

#[test]
fn resolves_relative_rpc_fixture_path_from_config_location() {
    let tempdir = tempdir().expect("tempdir should be created");
    let fixtures_dir = tempdir.path().join("fixtures");
    std::fs::create_dir_all(&fixtures_dir).expect("fixtures dir should be created");
    let path = tempdir.path().join("history.toml");

    fs::write(
        &path,
        r#"
[source]
kind = "rpc_range"
rpc_url = "fixtures/sample_rpc.json"
start_block = 100
end_block = 120
contract_address = "0x1111111111111111111111111111111111111111"
"#,
    )
    .expect("config should be written");

    let config = load_history_config(&path).expect("config should parse");

    match config.source {
        dex_sim::config::HistorySourceConfig::RpcRange { rpc_url, .. } => {
            assert_eq!(
                rpc_url,
                fixtures_dir.join("sample_rpc.json").display().to_string()
            );
        }
        _ => panic!("expected rpc_range source"),
    }
}
