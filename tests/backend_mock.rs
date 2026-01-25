use dex_sim::backend::{CallRequest, CallResponse, EvmBackend, MockBackend, TxRequest, TxResponse};

#[test]
fn send_tx_records_request_and_uses_scripted_response() {
    let mut backend = MockBackend::new();
    let request = TxRequest {
        to: "0x1111111111111111111111111111111111111111".to_string(),
        data: vec![0xde, 0xad, 0xbe, 0xef],
        value: 42,
    };

    backend.queue_tx_response(TxResponse {
        tx_hash: "0xabc".to_string(),
        success: true,
        gas_used: 55_555,
        output: vec![1, 2, 3],
    });

    let response = backend.send_tx(request.clone()).expect("tx should succeed");

    assert_eq!(
        response,
        TxResponse {
            tx_hash: "0xabc".to_string(),
            success: true,
            gas_used: 55_555,
            output: vec![1, 2, 3],
        }
    );
    assert_eq!(backend.sent_txs(), &[request]);
    assert_eq!(backend.pending_transaction_count(), 1);
}

#[test]
fn call_records_request_and_returns_scripted_response() {
    let mut backend = MockBackend::new();
    let request = CallRequest {
        to: "0x2222222222222222222222222222222222222222".to_string(),
        data: vec![0xca, 0xfe],
    };

    backend.queue_call_response(CallResponse {
        return_data: vec![9, 8, 7],
    });

    let response = backend.call(request.clone()).expect("call should succeed");

    assert_eq!(
        response,
        CallResponse {
            return_data: vec![9, 8, 7],
        }
    );
    assert_eq!(backend.calls(), &[request]);
}

#[test]
fn mine_block_advances_height_and_flushes_pending_transactions() {
    let mut backend = MockBackend::with_block_number(100);

    backend
        .send_tx(TxRequest {
            to: "0x1111111111111111111111111111111111111111".to_string(),
            data: vec![1],
            value: 1,
        })
        .expect("first tx should succeed");
    backend
        .send_tx(TxRequest {
            to: "0x2222222222222222222222222222222222222222".to_string(),
            data: vec![2],
            value: 2,
        })
        .expect("second tx should succeed");

    let mined_block = backend.mine_block().expect("mining should succeed");

    assert_eq!(mined_block.block_number, 101);
    assert_eq!(mined_block.transaction_count, 2);
    assert_eq!(backend.current_block_number(), 101);
    assert_eq!(backend.pending_transaction_count(), 0);
    assert_eq!(backend.mined_blocks(), &[mined_block]);
}

#[test]
fn backend_can_return_scripted_errors() {
    let mut backend = MockBackend::new();

    backend.queue_tx_error("send failed");
    backend.queue_call_error("call failed");
    backend.queue_mine_error("mine failed");

    let tx_error = backend
        .send_tx(TxRequest {
            to: "0x3333333333333333333333333333333333333333".to_string(),
            data: vec![],
            value: 0,
        })
        .expect_err("send_tx should fail");
    let call_error = backend
        .call(CallRequest {
            to: "0x4444444444444444444444444444444444444444".to_string(),
            data: vec![],
        })
        .expect_err("call should fail");
    let mine_error = backend.mine_block().expect_err("mine should fail");

    assert!(tx_error.to_string().contains("send failed"));
    assert!(call_error.to_string().contains("call failed"));
    assert!(mine_error.to_string().contains("mine failed"));
    assert_eq!(backend.current_block_number(), 0);
    assert_eq!(backend.pending_transaction_count(), 1);
}
