use dex_sim::{
    backend::{MockBackend, TxResponse},
    config::{DexAssets, DexConfig, DexContracts},
    dex::{
        build_adapter, traits::DexSwapAdapter, ExampleCustomAdapter, RegisteredDexAdapter,
        V2Adapter,
    },
    types::{SwapRequest, SwapStatus},
};

#[test]
fn registry_builds_custom_adapter_from_config() {
    let adapter = build_adapter(sample_custom_config()).expect("custom adapter should build");

    assert!(matches!(adapter, RegisteredDexAdapter::ExampleCustom(_)));
    assert_eq!(adapter.adapter_name(), "custom.example");
}

#[test]
fn custom_adapter_can_be_created_and_called() {
    let adapter =
        ExampleCustomAdapter::from_config(sample_custom_config()).expect("adapter should build");
    let swap = sample_swap_request();
    let mut backend = MockBackend::new();

    backend.queue_tx_response(TxResponse {
        tx_hash: "0xcustom".to_string(),
        success: true,
        gas_used: 77_777,
        output: b"1234567".to_vec(),
    });

    let result = adapter
        .execute_swap(&mut backend, &swap, 1)
        .expect("execution should succeed");

    assert_eq!(result.status, SwapStatus::Success);
    assert_eq!(result.amount_out.as_deref(), Some("1234567"));
    assert_eq!(result.gas_used, Some(77_777));

    let sent_txs = backend.sent_txs();
    assert_eq!(sent_txs.len(), 1);
    assert_eq!(sent_txs[0].to, "0x3333333333333333333333333333333333333333");

    let calldata = String::from_utf8(sent_txs[0].data.clone()).expect("payload should be utf-8");
    assert!(calldata.contains("custom_swap"));
    assert!(calldata.contains("pair="));
}

#[test]
fn custom_logic_overrides_builtin_behavior() {
    let custom = ExampleCustomAdapter::from_config(sample_custom_config())
        .expect("custom adapter should build");
    let builtin = V2Adapter::from_config(sample_builtin_config()).expect("builtin should build");
    let swap = sample_swap_request();

    let mut custom_backend = MockBackend::new();
    custom_backend.queue_tx_response(TxResponse {
        tx_hash: "0xc1".to_string(),
        success: true,
        gas_used: 50_000,
        output: Vec::new(),
    });

    let mut builtin_backend = MockBackend::new();
    builtin_backend.queue_tx_response(TxResponse {
        tx_hash: "0xb1".to_string(),
        success: true,
        gas_used: 50_000,
        output: Vec::new(),
    });

    let custom_result = custom
        .execute_swap(&mut custom_backend, &swap, 0)
        .expect("custom execution should succeed");
    let builtin_result = builtin
        .execute_swap(&mut builtin_backend, &swap, 0)
        .expect("builtin execution should succeed");

    assert_eq!(custom_result.status, SwapStatus::Success);
    assert_eq!(builtin_result.status, SwapStatus::Success);
    assert_eq!(custom_result.amount_out.as_deref(), Some("990000"));
    assert_eq!(builtin_result.amount_out, None);

    let custom_payload =
        String::from_utf8(custom_backend.sent_txs()[0].data.clone()).expect("utf-8 payload");
    let builtin_payload =
        String::from_utf8(builtin_backend.sent_txs()[0].data.clone()).expect("utf-8 payload");

    assert!(custom_payload.starts_with("custom_swap|"));
    assert!(builtin_payload.starts_with("swap_exact_input|"));
}

#[test]
fn custom_adapter_rejects_zero_amount_in() {
    let adapter =
        ExampleCustomAdapter::from_config(sample_custom_config()).expect("adapter should build");
    let mut swap = sample_swap_request();
    swap.amount_in = "0".to_string();

    let error = adapter
        .validate_swap(&swap)
        .expect_err("zero amount should fail validation");

    assert!(error.to_string().contains("amount_in"));
}

fn sample_custom_config() -> DexConfig {
    DexConfig {
        name: "my-custom-dex".to_string(),
        adapter: "custom.example".to_string(),
        contracts: DexContracts {
            router: "0x3333333333333333333333333333333333333333".to_string(),
            factory: None,
            quoter: None,
        },
        assets: DexAssets::default(),
        tokens: Vec::new(),
        protocol: Default::default(),
    }
}

fn sample_builtin_config() -> DexConfig {
    DexConfig {
        name: "mini-v2".to_string(),
        adapter: "builtin.v2".to_string(),
        contracts: DexContracts {
            router: "0x1111111111111111111111111111111111111111".to_string(),
            factory: Some("0x2222222222222222222222222222222222222222".to_string()),
            quoter: None,
        },
        assets: DexAssets::default(),
        tokens: vec![
            dex_sim::config::TokenConfig {
                symbol: "USDC".to_string(),
                address: "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
                decimals: 6,
            },
            dex_sim::config::TokenConfig {
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
