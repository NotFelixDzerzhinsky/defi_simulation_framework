use dex_sim::{
    backend::{MockBackend, TxResponse},
    config::{DexAssets, DexConfig, DexContracts, TokenConfig},
    dex::{build_adapter, traits::DexSwapAdapter, RegisteredDexAdapter, V2Adapter},
    types::{SwapRequest, SwapStatus},
};

#[test]
fn registry_builds_builtin_v2_adapter_from_config() {
    let adapter = build_adapter(sample_dex_config()).expect("adapter should build");

    assert!(matches!(adapter, RegisteredDexAdapter::BuiltinV2(_)));
    assert_eq!(adapter.adapter_name(), "builtin.v2");
}

#[test]
fn v2_adapter_rejects_swap_with_unknown_token() {
    let adapter = V2Adapter::from_config(sample_dex_config()).expect("adapter should build");
    let mut swap = sample_swap_request();
    swap.token_out = "0x9999999999999999999999999999999999999999".to_string();

    let error = adapter
        .validate_swap(&swap)
        .expect_err("swap should fail validation");

    assert!(error.to_string().contains("token_out"));
}

#[test]
fn v2_adapter_executes_swap_via_mock_backend() {
    let adapter = V2Adapter::from_config(sample_dex_config()).expect("adapter should build");
    let swap = sample_swap_request();
    let mut backend = MockBackend::new();

    backend.queue_tx_response(TxResponse {
        tx_hash: "0xfeed".to_string(),
        success: true,
        gas_used: 91_000,
        output: b"990000".to_vec(),
    });

    let result = adapter
        .execute_swap(&mut backend, &swap, 3)
        .expect("execution should not fail");

    assert_eq!(result.block_number, swap.block_number);
    assert_eq!(result.swap_index, 3);
    assert_eq!(result.status, SwapStatus::Success);
    assert_eq!(result.amount_out.as_deref(), Some("990000"));
    assert_eq!(result.gas_used, Some(91_000));
    assert!(result.error.is_none());

    let sent_txs = backend.sent_txs();
    assert_eq!(sent_txs.len(), 1);
    assert_eq!(sent_txs[0].to, "0x1111111111111111111111111111111111111111");

    let calldata = String::from_utf8(sent_txs[0].data.clone()).expect("payload should be utf-8");
    assert!(calldata.contains("swap_exact_input"));
    assert!(calldata.contains("amount_in=1000000"));
    assert_eq!(backend.pending_transaction_count(), 1);
}

#[test]
fn v2_adapter_maps_backend_error_to_revert_result() {
    let adapter = V2Adapter::from_config(sample_dex_config()).expect("adapter should build");
    let swap = sample_swap_request();
    let mut backend = MockBackend::new();

    backend.queue_tx_error("router execution failed");

    let result = adapter
        .execute_swap(&mut backend, &swap, 0)
        .expect("backend errors should become normalized results");

    assert_eq!(result.status, SwapStatus::Revert);
    assert!(result.amount_out.is_none());
    assert!(result.gas_used.is_none());
    assert!(result
        .error
        .as_deref()
        .expect("error should be present")
        .contains("router execution failed"));
}

fn sample_dex_config() -> DexConfig {
    DexConfig {
        name: "mini-v2".to_string(),
        adapter: "builtin.v2".to_string(),
        contracts: DexContracts {
            router: "0x1111111111111111111111111111111111111111".to_string(),
            factory: Some("0x2222222222222222222222222222222222222222".to_string()),
            quoter: None,
        },
        fee_bps: None,
        assets: DexAssets::default(),
        tokens: vec![
            TokenConfig {
                symbol: "USDC".to_string(),
                address: "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
                decimals: 6,
            },
            TokenConfig {
                symbol: "WETH".to_string(),
                address: "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
                decimals: 18,
            },
        ],
        protocol: Default::default(),
    }
}

fn sample_swap_request() -> SwapRequest {
    SwapRequest {
        block_number: 123,
        sender: "0xcccccccccccccccccccccccccccccccccccccccc".to_string(),
        token_in: "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        token_out: "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
        amount_in: "1000000".to_string(),
        min_amount_out: "990000".to_string(),
        tx_hash: Some("0x1234".to_string()),
        tx_index: Some(0),
        metadata: Default::default(),
    }
}
