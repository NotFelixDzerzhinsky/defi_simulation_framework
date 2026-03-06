use std::{fs, process::Command};

use dex_sim::{
    backend::{AnvilBackend, EvmBackend, TxRequest},
    config::HistorySourceConfig,
    history::{RpcRangeSource, SwapInputSource},
};
use tempfile::tempdir;

fn has_anvil() -> bool {
    Command::new("anvil")
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

#[test]
fn rpc_range_source_filters_transactions_and_groups_by_block() {
    let tempdir = tempdir().expect("tempdir should be created");
    let path = tempdir.path().join("rpc_fixture.json");

    fs::write(
        &path,
        r#"{
  "transactions": [
    {
      "block_number": 99,
      "to": "0x1111111111111111111111111111111111111111",
      "method": "swapExactTokensForTokens",
      "sender": "0xaaaa",
      "token_in": "0x1111",
      "token_out": "0x2222",
      "amount_in": "10",
      "min_amount_out": "9",
      "tx_hash": "0x1",
      "tx_index": 0
    },
    {
      "block_number": 100,
      "to": "0x1111111111111111111111111111111111111111",
      "method": "swapExactTokensForTokens",
      "sender": "0xbbbb",
      "token_in": "0x1111",
      "token_out": "0x3333",
      "amount_in": "20",
      "min_amount_out": "19",
      "tx_hash": "0x2",
      "tx_index": 0
    },
    {
      "block_number": 101,
      "to": "0x9999999999999999999999999999999999999999",
      "method": "swapExactTokensForTokens",
      "sender": "0xcccc",
      "token_in": "0x1111",
      "token_out": "0x4444",
      "amount_in": "30",
      "min_amount_out": "29",
      "tx_hash": "0x3",
      "tx_index": 0
    },
    {
      "block_number": 101,
      "to": "0x1111111111111111111111111111111111111111",
      "method": "otherMethod",
      "sender": "0xdddd",
      "token_in": "0x1111",
      "token_out": "0x5555",
      "amount_in": "40",
      "min_amount_out": "39",
      "tx_hash": "0x4",
      "tx_index": 0
    },
    {
      "block_number": 101,
      "to": "0x1111111111111111111111111111111111111111",
      "method": "swapExactTokensForTokens",
      "sender": "0xeeee",
      "token_in": "0x2222",
      "token_out": "0x3333",
      "amount_in": "50",
      "min_amount_out": "49",
      "tx_hash": "0x5",
      "tx_index": 1
    }
  ]
}"#,
    )
    .expect("fixture should be written");

    let mut source = RpcRangeSource::from_fixture_path(
        &path.display().to_string(),
        100,
        101,
        "0x1111111111111111111111111111111111111111",
        Some("swapExactTokensForTokens"),
    )
    .expect("source should build");

    let blocks = source.collect_all().expect("source should produce blocks");

    assert_eq!(blocks.len(), 2);
    assert_eq!(blocks[0].block_number, 100);
    assert_eq!(blocks[0].swaps.len(), 1);
    assert_eq!(blocks[0].swaps[0].sender, "0xbbbb");
    assert_eq!(blocks[1].block_number, 101);
    assert_eq!(blocks[1].swaps.len(), 1);
    assert_eq!(blocks[1].swaps[0].sender, "0xeeee");
}

#[test]
fn rpc_range_source_builds_from_config_with_file_url() {
    let tempdir = tempdir().expect("tempdir should be created");
    let path = tempdir.path().join("rpc_fixture.json");

    fs::write(
        &path,
        r#"{
  "transactions": [
    {
      "block_number": 100,
      "to": "0x1111111111111111111111111111111111111111",
      "method": "swapExactTokensForTokens",
      "sender": "0xbbbb",
      "token_in": "0x1111",
      "token_out": "0x3333",
      "amount_in": "20",
      "min_amount_out": "19"
    }
  ]
}"#,
    )
    .expect("fixture should be written");

    let config = HistorySourceConfig::RpcRange {
        rpc_url: format!("file://{}", path.display()),
        start_block: 100,
        end_block: 100,
        contract_address: "0x1111111111111111111111111111111111111111".to_string(),
        method: Some("swapExactTokensForTokens".to_string()),
    };

    let mut source = RpcRangeSource::from_config(&config).expect("source should build");
    let block = source
        .next_block()
        .expect("source should work")
        .expect("block should exist");

    assert_eq!(block.block_number, 100);
    assert_eq!(block.swaps.len(), 1);
}

#[test]
fn rpc_range_source_fetches_live_http_rpc_from_anvil() {
    if !has_anvil() {
        return;
    }

    let router = "0x1111111111111111111111111111111111111111";
    let mut source_chain = AnvilBackend::spawn().expect("anvil backend should start");

    source_chain
        .send_tx(TxRequest {
            to: router.to_string(),
            data: b"swap_exact_input|sender=0xaaaa|token_in=0x1111|token_out=0x2222|amount_in=10|min_amount_out=9".to_vec(),
            value: 0,
        })
        .expect("first tx should succeed");
    source_chain
        .send_tx(TxRequest {
            to: "0x9999999999999999999999999999999999999999".to_string(),
            data: b"swap_exact_input|sender=0xbbbb|token_in=0x1111|token_out=0x3333|amount_in=20|min_amount_out=19".to_vec(),
            value: 0,
        })
        .expect("second tx should succeed");
    source_chain
        .send_tx(TxRequest {
            to: router.to_string(),
            data: b"swap_exact_input|sender=0xcccc|token_in=0x2222|token_out=0x3333|amount_in=30|min_amount_out=29".to_vec(),
            value: 0,
        })
        .expect("third tx should succeed");

    let config = HistorySourceConfig::RpcRange {
        rpc_url: source_chain.rpc_url().to_string(),
        start_block: 1,
        end_block: 3,
        contract_address: router.to_string(),
        method: Some("swap_exact_input".to_string()),
    };

    let mut source = RpcRangeSource::from_config(&config).expect("live rpc source should build");
    let blocks = source.collect_all().expect("source should produce blocks");

    assert_eq!(blocks.len(), 2);
    assert_eq!(blocks[0].block_number, 1);
    assert_eq!(blocks[0].swaps[0].sender, "0xaaaa");
    assert_eq!(blocks[1].block_number, 3);
    assert_eq!(blocks[1].swaps[0].sender, "0xcccc");
}

#[test]
fn rpc_range_source_reports_clear_error_for_bad_live_payload() {
    if !has_anvil() {
        return;
    }

    let router = "0x1111111111111111111111111111111111111111";
    let mut source_chain = AnvilBackend::spawn().expect("anvil backend should start");

    source_chain
        .send_tx(TxRequest {
            to: router.to_string(),
            data: b"not-a-decodable-swap-payload".to_vec(),
            value: 0,
        })
        .expect("tx should succeed");

    let config = HistorySourceConfig::RpcRange {
        rpc_url: source_chain.rpc_url().to_string(),
        start_block: 1,
        end_block: 1,
        contract_address: router.to_string(),
        method: Some("swap_exact_input".to_string()),
    };

    let error = RpcRangeSource::from_config(&config).expect_err("bad live payload should fail");

    assert!(error.to_string().contains("failed to decode swap payload"));
}
