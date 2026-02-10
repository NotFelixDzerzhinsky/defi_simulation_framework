use std::fs;

use dex_sim::{
    config::HistorySourceConfig,
    history::{JsonlSource, SwapInputSource},
};
use tempfile::tempdir;

#[test]
fn jsonl_source_reads_and_groups_swaps_by_block() {
    let tempdir = tempdir().expect("tempdir should be created");
    let path = tempdir.path().join("swaps.jsonl");

    fs::write(
        &path,
        r#"{"block_number":101,"sender":"0xaaaa","token_in":"0x1111","token_out":"0x2222","amount_in":"10","min_amount_out":"9","tx_hash":"0x1","tx_index":0}
{"block_number":100,"sender":"0xbbbb","token_in":"0x1111","token_out":"0x3333","amount_in":"20","min_amount_out":"19","tx_hash":"0x2","tx_index":0}

{"block_number":101,"sender":"0xcccc","token_in":"0x2222","token_out":"0x3333","amount_in":"30","min_amount_out":"29","tx_hash":"0x3","tx_index":1}
"#,
    )
    .expect("jsonl should be written");

    let mut source = JsonlSource::from_path(&path).expect("source should build");
    let blocks = source.collect_all().expect("source should read blocks");

    assert_eq!(blocks.len(), 2);
    assert_eq!(blocks[0].block_number, 100);
    assert_eq!(blocks[0].swaps.len(), 1);
    assert_eq!(blocks[1].block_number, 101);
    assert_eq!(blocks[1].swaps.len(), 2);
    assert_eq!(blocks[1].swaps[0].sender, "0xaaaa");
    assert_eq!(blocks[1].swaps[1].sender, "0xcccc");
}

#[test]
fn jsonl_source_builds_from_history_config() {
    let tempdir = tempdir().expect("tempdir should be created");
    let path = tempdir.path().join("swaps.jsonl");

    fs::write(
        &path,
        r#"{"block_number":42,"sender":"0xaaaa","token_in":"0x1111","token_out":"0x2222","amount_in":"1","min_amount_out":"1"}"#,
    )
    .expect("jsonl should be written");

    let config = HistorySourceConfig::Jsonl { path: path.clone() };
    let mut source = JsonlSource::from_config(&config).expect("source should build");

    let block = source
        .next_block()
        .expect("source should work")
        .expect("block should exist");

    assert_eq!(block.block_number, 42);
    assert_eq!(block.swaps.len(), 1);
    assert!(source
        .next_block()
        .expect("second read should work")
        .is_none());
}

#[test]
fn jsonl_source_rejects_invalid_json_line() {
    let tempdir = tempdir().expect("tempdir should be created");
    let path = tempdir.path().join("broken.jsonl");

    fs::write(
        &path,
        r#"{"block_number":42,"sender":"0xaaaa","token_in":"0x1111","token_out":"0x2222","amount_in":"1","min_amount_out":"1"}
not-json
"#,
    )
    .expect("jsonl should be written");

    let error = JsonlSource::from_path(&path).expect_err("broken jsonl should fail");

    assert!(error.to_string().contains("line 2"));
}
