use crate::error::AppResult;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TxRequest {
    pub to: String,
    pub data: Vec<u8>,
    pub value: u128,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallRequest {
    pub to: String,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TxResponse {
    pub tx_hash: String,
    pub success: bool,
    pub gas_used: u64,
    pub output: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CallResponse {
    pub return_data: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinedBlock {
    pub block_number: u64,
    pub transaction_count: usize,
}

pub trait EvmBackend {
    fn send_tx(&mut self, request: TxRequest) -> AppResult<TxResponse>;
    fn call(&mut self, request: CallRequest) -> AppResult<CallResponse>;
    fn mine_block(&mut self) -> AppResult<MinedBlock>;
    fn current_block_number(&self) -> u64;
}
