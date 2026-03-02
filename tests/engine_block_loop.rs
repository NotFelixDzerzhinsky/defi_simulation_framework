use dex_sim::{
    backend::{EvmBackend, MockBackend, TxResponse},
    config::{DexAssets, DexConfig, DexContracts, ExecutionConfig, TokenConfig},
    dex::{traits::DexSwapAdapter, V2Adapter},
    engine::EngineRunner,
    history::JsonlSource,
};

#[test]
fn engine_executes_swaps_in_block_order_and_mines_blocks() {
    let adapter = V2Adapter::from_config(sample_dex_config()).expect("adapter should build");
    let mut source = JsonlSource::from_swaps(sample_swaps()).expect("source should build");
    let mut backend = MockBackend::new();

    backend.queue_tx_response(TxResponse {
        tx_hash: "0x1".to_string(),
        success: true,
        gas_used: 11_000,
        output: b"91".to_vec(),
    });
    backend.queue_tx_response(TxResponse {
        tx_hash: "0x2".to_string(),
        success: true,
        gas_used: 12_000,
        output: b"181".to_vec(),
    });
    backend.queue_tx_response(TxResponse {
        tx_hash: "0x3".to_string(),
        success: true,
        gas_used: 13_000,
        output: b"271".to_vec(),
    });

    let report = EngineRunner::run(
        &mut backend,
        &adapter,
        &mut source,
        &ExecutionConfig {
            mine_after_block: true,
            continue_on_revert: true,
        },
    )
    .expect("engine should run");

    assert_eq!(report.blocks.len(), 2);
    assert_eq!(report.blocks[0].block_number, 100);
    assert_eq!(report.blocks[0].swaps.len(), 2);
    assert_eq!(report.blocks[1].block_number, 101);
    assert_eq!(report.blocks[1].swaps.len(), 1);

    assert_eq!(report.summary.processed_blocks, 2);
    assert_eq!(report.summary.total_swaps, 3);
    assert_eq!(report.summary.successes, 3);
    assert_eq!(report.summary.reverts, 0);
    assert_eq!(report.summary.skipped, 0);
    assert_eq!(report.summary.total_gas_used, 36_000);

    let calldata_0 =
        String::from_utf8(backend.sent_txs()[0].data.clone()).expect("payload should be utf-8");
    let calldata_1 =
        String::from_utf8(backend.sent_txs()[1].data.clone()).expect("payload should be utf-8");
    let calldata_2 =
        String::from_utf8(backend.sent_txs()[2].data.clone()).expect("payload should be utf-8");

    assert!(calldata_0.contains("amount_in=100"));
    assert!(calldata_1.contains("amount_in=200"));
    assert!(calldata_2.contains("amount_in=300"));
    assert_eq!(backend.current_block_number(), 2);
    assert_eq!(backend.mined_blocks().len(), 2);
    assert_eq!(backend.mined_blocks()[0].transaction_count, 2);
    assert_eq!(backend.mined_blocks()[1].transaction_count, 1);
}

#[test]
fn engine_stops_within_block_after_revert_when_continue_on_revert_is_false() {
    let adapter = V2Adapter::from_config(sample_dex_config()).expect("adapter should build");
    let mut source = JsonlSource::from_swaps(vec![
        sample_swap(100, "100", "91"),
        sample_swap(100, "200", "181"),
        sample_swap(100, "300", "271"),
    ])
    .expect("source should build");
    let mut backend = MockBackend::new();

    backend.queue_tx_response(TxResponse {
        tx_hash: "0x1".to_string(),
        success: true,
        gas_used: 11_000,
        output: b"91".to_vec(),
    });
    backend.queue_tx_response(TxResponse {
        tx_hash: "0x2".to_string(),
        success: false,
        gas_used: 12_000,
        output: b"router revert".to_vec(),
    });

    let report = EngineRunner::run(
        &mut backend,
        &adapter,
        &mut source,
        &ExecutionConfig {
            mine_after_block: true,
            continue_on_revert: false,
        },
    )
    .expect("engine should run");

    assert_eq!(report.blocks.len(), 1);
    assert_eq!(report.blocks[0].swaps.len(), 3);
    assert!(report.blocks[0].swaps[0].is_success());
    assert_eq!(
        report.blocks[0].swaps[1].error.as_deref(),
        Some("swap reverted")
    );
    assert_eq!(
        report.blocks[0].swaps[2].error.as_deref(),
        Some("skipped because a previous swap in the block reverted")
    );

    assert_eq!(report.summary.successes, 1);
    assert_eq!(report.summary.reverts, 1);
    assert_eq!(report.summary.skipped, 1);
    assert_eq!(backend.sent_txs().len(), 2);
    assert_eq!(backend.mined_blocks().len(), 1);
}

#[test]
fn engine_can_skip_block_mining_when_disabled() {
    let adapter = V2Adapter::from_config(sample_dex_config()).expect("adapter should build");
    let mut source =
        JsonlSource::from_swaps(vec![sample_swap(100, "100", "91")]).expect("source should build");
    let mut backend = MockBackend::with_block_number(10);

    backend.queue_tx_response(TxResponse {
        tx_hash: "0x1".to_string(),
        success: true,
        gas_used: 11_000,
        output: b"91".to_vec(),
    });

    let report = EngineRunner::run(
        &mut backend,
        &adapter,
        &mut source,
        &ExecutionConfig {
            mine_after_block: false,
            continue_on_revert: true,
        },
    )
    .expect("engine should run");

    assert_eq!(report.summary.processed_blocks, 1);
    assert_eq!(backend.current_block_number(), 10);
    assert!(backend.mined_blocks().is_empty());
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

fn sample_swaps() -> Vec<dex_sim::types::SwapRequest> {
    vec![
        sample_swap(100, "100", "91"),
        sample_swap(100, "200", "181"),
        sample_swap(101, "300", "271"),
    ]
}

fn sample_swap(
    block_number: u64,
    amount_in: &str,
    min_amount_out: &str,
) -> dex_sim::types::SwapRequest {
    dex_sim::types::SwapRequest {
        block_number,
        sender: "0xcccccccccccccccccccccccccccccccccccccccc".to_string(),
        token_in: "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        token_out: "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
        amount_in: amount_in.to_string(),
        min_amount_out: min_amount_out.to_string(),
        tx_hash: None,
        tx_index: None,
        metadata: Default::default(),
    }
}
