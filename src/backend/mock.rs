use std::collections::VecDeque;

use crate::{
    backend::evm::{CallRequest, CallResponse, EvmBackend, MinedBlock, TxRequest, TxResponse},
    error::{AppError, AppResult},
};

#[derive(Debug, Default)]
pub struct MockBackend {
    current_block_number: u64,
    sent_txs: Vec<TxRequest>,
    pending_txs: Vec<TxRequest>,
    calls: Vec<CallRequest>,
    mined_blocks: Vec<MinedBlock>,
    scripted_tx_results: VecDeque<AppResult<TxResponse>>,
    scripted_call_results: VecDeque<AppResult<CallResponse>>,
    scripted_mine_results: VecDeque<AppResult<()>>,
}

impl MockBackend {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_block_number(block_number: u64) -> Self {
        Self {
            current_block_number: block_number,
            ..Self::default()
        }
    }

    pub fn queue_tx_response(&mut self, response: TxResponse) {
        self.scripted_tx_results.push_back(Ok(response));
    }

    pub fn queue_tx_error(&mut self, message: impl Into<String>) {
        self.scripted_tx_results
            .push_back(Err(AppError::backend(message.into())));
    }

    pub fn queue_call_response(&mut self, response: CallResponse) {
        self.scripted_call_results.push_back(Ok(response));
    }

    pub fn queue_call_error(&mut self, message: impl Into<String>) {
        self.scripted_call_results
            .push_back(Err(AppError::backend(message.into())));
    }

    pub fn queue_mine_error(&mut self, message: impl Into<String>) {
        self.scripted_mine_results
            .push_back(Err(AppError::backend(message.into())));
    }

    pub fn sent_txs(&self) -> &[TxRequest] {
        &self.sent_txs
    }

    pub fn calls(&self) -> &[CallRequest] {
        &self.calls
    }

    pub fn mined_blocks(&self) -> &[MinedBlock] {
        &self.mined_blocks
    }

    pub fn pending_transaction_count(&self) -> usize {
        self.pending_txs.len()
    }

    fn default_tx_response(&self) -> TxResponse {
        TxResponse {
            tx_hash: format!("0xmock-tx-{:x}", self.sent_txs.len().saturating_sub(1)),
            success: true,
            gas_used: 21_000,
            output: Vec::new(),
        }
    }
}

impl EvmBackend for MockBackend {
    fn send_tx(&mut self, request: TxRequest) -> AppResult<TxResponse> {
        self.sent_txs.push(request.clone());
        self.pending_txs.push(request);

        if let Some(result) = self.scripted_tx_results.pop_front() {
            return result;
        }

        Ok(self.default_tx_response())
    }

    fn call(&mut self, request: CallRequest) -> AppResult<CallResponse> {
        self.calls.push(request);

        if let Some(result) = self.scripted_call_results.pop_front() {
            return result;
        }

        Ok(CallResponse::default())
    }

    fn mine_block(&mut self) -> AppResult<MinedBlock> {
        if let Some(result) = self.scripted_mine_results.pop_front() {
            result?;
        }

        self.current_block_number += 1;

        let mined_block = MinedBlock {
            block_number: self.current_block_number,
            transaction_count: self.pending_txs.len(),
        };

        self.pending_txs.clear();
        self.mined_blocks.push(mined_block.clone());

        Ok(mined_block)
    }

    fn current_block_number(&self) -> u64 {
        self.current_block_number
    }
}
