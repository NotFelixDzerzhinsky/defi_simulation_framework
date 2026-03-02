use std::process::Command;

use dex_sim::backend::{AnvilBackend, CallRequest, EvmBackend, TxRequest};

fn has_anvil() -> bool {
    Command::new("anvil")
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

#[test]
fn send_tx_works_on_real_anvil() {
    if !has_anvil() {
        return;
    }

    let mut backend = AnvilBackend::spawn().expect("anvil backend should start");
    let recipient = backend
        .accounts()
        .get(1)
        .expect("anvil should expose at least two accounts")
        .clone();

    let response = backend
        .send_tx(TxRequest {
            to: recipient,
            data: Vec::new(),
            value: 1,
        })
        .expect("transaction should succeed");

    assert!(response.success);
    assert!(response.tx_hash.starts_with("0x"));
    assert!(response.gas_used > 0);
    assert!(backend.current_block_number() >= 1);
}

#[test]
fn call_works_on_real_anvil() {
    if !has_anvil() {
        return;
    }

    let mut backend = AnvilBackend::spawn().expect("anvil backend should start");
    let recipient = backend
        .accounts()
        .get(1)
        .expect("anvil should expose at least two accounts")
        .clone();

    let response = backend
        .call(CallRequest {
            to: recipient,
            data: Vec::new(),
        })
        .expect("eth_call should succeed");

    assert!(response.return_data.is_empty());
}

#[test]
fn mine_block_works_on_real_anvil() {
    if !has_anvil() {
        return;
    }

    let mut backend = AnvilBackend::spawn().expect("anvil backend should start");
    let before = backend.current_block_number();

    let mined = backend.mine_block().expect("mine should succeed");

    assert_eq!(mined.block_number, before + 1);
    assert_eq!(backend.current_block_number(), before + 1);
}
