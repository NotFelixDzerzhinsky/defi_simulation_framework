use std::fs;

use dex_sim::{
    output::{render_report_summary, serialize_report, write_report, write_report_summary},
    types::{BlockExecutionResult, RunReport, SwapExecutionResult, SwapStatus},
};
use tempfile::tempdir;

#[test]
fn serialize_report_flattens_block_results_into_jsonl() {
    let report = sample_report();

    let jsonl = serialize_report(&report).expect("report should serialize");
    let lines = jsonl.lines().collect::<Vec<_>>();

    assert_eq!(lines.len(), 3);
    assert!(lines[0].contains("\"block_number\":100"));
    assert!(lines[0].contains("\"swap_index\":0"));
    assert!(lines[1].contains("\"status\":\"revert\""));
    assert!(lines[2].contains("\"status\":\"skipped\""));
}

#[test]
fn write_report_persists_raw_log_to_disk() {
    let report = sample_report();
    let tempdir = tempdir().expect("tempdir should be created");
    let path = tempdir.path().join("nested/output.raw.jsonl");

    let written_path = write_report(&path, &report).expect("report should be written");
    let content = fs::read_to_string(&written_path).expect("raw log should exist");

    assert_eq!(written_path, path);
    assert_eq!(content.lines().count(), 3);
    assert!(content.contains("\"status\":\"success\""));
    assert!(content.contains("\"status\":\"revert\""));
}

#[test]
fn write_report_summary_persists_summary_text_to_disk() {
    let report = sample_report();
    let tempdir = tempdir().expect("tempdir should be created");
    let path = tempdir.path().join("nested/output.summary.txt");

    let written_path =
        write_report_summary(&path, &report).expect("summary should be written to disk");
    let content = fs::read_to_string(&written_path).expect("summary file should exist");

    assert_eq!(written_path, path);
    assert_eq!(content, render_report_summary(&report));
    assert!(content.contains("processed_blocks=2"));
    assert!(content.contains("total_swaps=3"));
    assert!(content.contains("successes=1"));
    assert!(content.contains("reverts=1"));
    assert!(content.contains("skipped=1"));
    assert!(content.contains("total_gas_used=11000"));
}

fn sample_report() -> RunReport {
    let mut report = RunReport::default();

    report.push_block(BlockExecutionResult {
        block_number: 100,
        swaps: vec![
            SwapExecutionResult {
                block_number: 100,
                swap_index: 0,
                status: SwapStatus::Success,
                amount_out: Some("91".to_string()),
                gas_used: Some(11_000),
                error: None,
            },
            SwapExecutionResult {
                block_number: 100,
                swap_index: 1,
                status: SwapStatus::Revert,
                amount_out: None,
                gas_used: None,
                error: Some("router revert".to_string()),
            },
        ],
    });

    report.push_block(BlockExecutionResult {
        block_number: 101,
        swaps: vec![SwapExecutionResult {
            block_number: 101,
            swap_index: 0,
            status: SwapStatus::Skipped,
            amount_out: None,
            gas_used: None,
            error: Some("skipped because a previous swap in the block reverted".to_string()),
        }],
    });

    report
}
