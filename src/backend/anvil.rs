use std::{
    io,
    net::TcpListener,
    path::PathBuf,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use serde_json::{json, Value};

use crate::{
    backend::evm::{CallRequest, CallResponse, EvmBackend, MinedBlock, TxRequest, TxResponse},
    error::{AppError, AppResult},
};

const DEFAULT_HOST: &str = "127.0.0.1";
const STARTUP_TIMEOUT: Duration = Duration::from_secs(30);
const RECEIPT_TIMEOUT: Duration = Duration::from_secs(60);
const POLL_INTERVAL: Duration = Duration::from_millis(200);

#[derive(Debug, Clone, Default)]
pub struct ForkConfig {
    pub fork_url: String,
    pub fork_block_number: Option<u64>,
}

#[derive(Debug)]
pub struct AnvilBackend {
    rpc_url: String,
    current_block_number: u64,
    accounts: Vec<String>,
    default_sender: String,
    child: Option<Child>,
}

impl AnvilBackend {
    pub fn new() -> AppResult<Self> {
        Self::spawn_inner(None)
    }

    pub fn new_forked(fork: ForkConfig) -> AppResult<Self> {
        Self::spawn_inner(Some(fork))
    }

    pub fn spawn() -> AppResult<Self> {
        Self::spawn_inner(None)
    }

    fn spawn_inner(fork: Option<ForkConfig>) -> AppResult<Self> {
        let port = pick_free_port()?;
        let rpc_url = format!("http://{DEFAULT_HOST}:{port}");

        let mut cmd = Command::new(anvil_binary());
        cmd.arg("--host")
            .arg(DEFAULT_HOST)
            .arg("--port")
            .arg(port.to_string())
            .arg("-q")
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        if let Some(ref fork_cfg) = fork {
            if fork_cfg.fork_url.is_empty() {
                return Err(AppError::validation(
                    "fork_url must not be empty when using fork mode",
                ));
            }
            cmd.arg("--fork-url").arg(&fork_cfg.fork_url);

            if let Some(block) = fork_cfg.fork_block_number {
                cmd.arg("--fork-block-number").arg(block.to_string());
            }
        }

        let child = cmd
            .spawn()
            .map_err(|source| AppError::backend(format!("failed to spawn anvil: {source}")))?;

        let mut backend = Self {
            rpc_url,
            current_block_number: 0,
            accounts: Vec::new(),
            default_sender: String::new(),
            child: Some(child),
        };

        backend.initialize()?;
        Ok(backend)
    }

    pub fn connect(rpc_url: impl Into<String>) -> AppResult<Self> {
        let mut backend = Self {
            rpc_url: rpc_url.into(),
            current_block_number: 0,
            accounts: Vec::new(),
            default_sender: String::new(),
            child: None,
        };

        backend.initialize()?;
        Ok(backend)
    }

    pub fn rpc_url(&self) -> &str {
        &self.rpc_url
    }

    pub fn accounts(&self) -> &[String] {
        &self.accounts
    }

    pub fn default_sender(&self) -> &str {
        &self.default_sender
    }

    fn initialize(&mut self) -> AppResult<()> {
        let deadline = Instant::now() + STARTUP_TIMEOUT;

        while Instant::now() < deadline {
            match self.fetch_accounts() {
                Ok(accounts) if !accounts.is_empty() => {
                    self.default_sender = accounts[0].clone();
                    self.accounts = accounts;
                    self.current_block_number = self.fetch_block_number()?;
                    return Ok(());
                }
                Ok(_) => thread::sleep(POLL_INTERVAL),
                Err(_) => thread::sleep(POLL_INTERVAL),
            }
        }

        Err(AppError::backend(format!(
            "anvil did not become ready at {} within {:?}",
            self.rpc_url, STARTUP_TIMEOUT
        )))
    }

    fn fetch_accounts(&self) -> AppResult<Vec<String>> {
        let value = self.rpc("eth_accounts", json!([]))?;
        let accounts: Vec<String> = serde_json::from_value(value).map_err(|error| {
            AppError::backend(format!("failed to decode eth_accounts response: {error}"))
        })?;
        Ok(accounts)
    }

    fn fetch_block_number(&self) -> AppResult<u64> {
        let value = self.rpc("eth_blockNumber", json!([]))?;
        parse_hex_u64(&value)
    }

    fn fetch_transaction_receipt(&self, tx_hash: &str) -> AppResult<Option<Value>> {
        let value = self.rpc("eth_getTransactionReceipt", json!([tx_hash]))?;

        if value.is_null() {
            Ok(None)
        } else {
            Ok(Some(value))
        }
    }

    fn fetch_block_by_number(&self, block_number: u64) -> AppResult<Value> {
        self.rpc(
            "eth_getBlockByNumber",
            json!([format!("0x{block_number:x}"), false]),
        )
    }

    fn wait_for_receipt(&self, tx_hash: &str) -> AppResult<Value> {
        let deadline = Instant::now() + RECEIPT_TIMEOUT;

        while Instant::now() < deadline {
            if let Some(receipt) = self.fetch_transaction_receipt(tx_hash)? {
                return Ok(receipt);
            }

            thread::sleep(POLL_INTERVAL);
        }

        Err(AppError::backend(format!(
            "timed out waiting for transaction receipt `{tx_hash}`"
        )))
    }

    fn rpc(&self, method: &str, params: Value) -> AppResult<Value> {
        let response = ureq::post(&self.rpc_url)
            .send_json(json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": method,
                "params": params,
            }))
            .map_err(|error| {
                AppError::backend(format!("rpc `{method}` request failed: {error}"))
            })?;

        let body: Value = response.into_json().map_err(|error| {
            AppError::backend(format!("failed to decode rpc `{method}` response: {error}"))
        })?;

        if let Some(error) = body.get("error") {
            return Err(AppError::backend(format!(
                "rpc `{method}` returned error: {error}"
            )));
        }

        body.get("result")
            .cloned()
            .ok_or_else(|| AppError::backend(format!("rpc `{method}` response is missing result")))
    }
}

impl EvmBackend for AnvilBackend {
    fn send_tx(&mut self, request: TxRequest) -> AppResult<TxResponse> {
        let tx_hash_value = self.rpc(
            "eth_sendTransaction",
            json!([{
                "from": self.default_sender,
                "to": request.to,
                "data": encode_hex(&request.data),
                "value": format!("0x{:x}", request.value),
            }]),
        )?;

        let tx_hash = tx_hash_value
            .as_str()
            .ok_or_else(|| AppError::backend("eth_sendTransaction did not return a tx hash"))?
            .to_string();

        let receipt = self.wait_for_receipt(&tx_hash)?;
        let status = parse_hex_bool(
            receipt
                .get("status")
                .ok_or_else(|| AppError::backend("transaction receipt is missing status"))?,
        )?;
        let gas_used = parse_hex_u64(
            receipt
                .get("gasUsed")
                .ok_or_else(|| AppError::backend("transaction receipt is missing gasUsed"))?,
        )?;
        self.current_block_number = parse_hex_u64(
            receipt
                .get("blockNumber")
                .ok_or_else(|| AppError::backend("transaction receipt is missing blockNumber"))?,
        )?;

        Ok(TxResponse {
            tx_hash,
            success: status,
            gas_used,
            output: Vec::new(),
        })
    }

    fn call(&mut self, request: CallRequest) -> AppResult<CallResponse> {
        let mut call_obj = json!({
            "from": self.default_sender,
            "to": request.to,
            "data": encode_hex(&request.data),
        });
        if let Some(value) = request.value {
            if value > 0 {
                call_obj["value"] = json!(format!("0x{:x}", value));
            }
        }
        let result = self.rpc(
            "eth_call",
            json!([call_obj, "latest"]),
        )?;

        Ok(CallResponse {
            return_data: decode_hex_bytes(&result)?,
        })
    }

    fn mine_block(&mut self) -> AppResult<MinedBlock> {
        self.rpc("anvil_mine", json!([1]))?;
        self.current_block_number = self.fetch_block_number()?;
        let block = self.fetch_block_by_number(self.current_block_number)?;
        let transaction_count = block
            .get("transactions")
            .and_then(Value::as_array)
            .map(|txs| txs.len())
            .unwrap_or_default();

        Ok(MinedBlock {
            block_number: self.current_block_number,
            transaction_count,
        })
    }

    fn current_block_number(&self) -> u64 {
        self.current_block_number
    }
}

impl Drop for AnvilBackend {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn parse_hex_u64(value: &Value) -> AppResult<u64> {
    let text = value
        .as_str()
        .ok_or_else(|| AppError::backend(format!("expected hex string, got `{value}`")))?;
    let stripped = text.trim_start_matches("0x");
    u64::from_str_radix(stripped, 16).map_err(|error| {
        AppError::backend(format!("failed to parse hex u64 value `{text}`: {error}"))
    })
}

fn parse_hex_bool(value: &Value) -> AppResult<bool> {
    Ok(parse_hex_u64(value)? != 0)
}

fn encode_hex(bytes: &[u8]) -> String {
    let mut output = String::from("0x");

    for byte in bytes {
        output.push_str(&format!("{byte:02x}"));
    }

    output
}

fn decode_hex_bytes(value: &Value) -> AppResult<Vec<u8>> {
    let text = value
        .as_str()
        .ok_or_else(|| AppError::backend(format!("expected hex string, got `{value}`")))?;
    let stripped = text.trim_start_matches("0x");

    if stripped.is_empty() {
        return Ok(Vec::new());
    }

    if stripped.len() % 2 != 0 {
        return Err(AppError::backend(format!(
            "hex payload must have even length, got `{text}`"
        )));
    }

    let mut output = Vec::with_capacity(stripped.len() / 2);
    let bytes = stripped.as_bytes();

    for index in (0..bytes.len()).step_by(2) {
        let chunk = std::str::from_utf8(&bytes[index..index + 2]).map_err(|error| {
            AppError::backend(format!("hex payload contains invalid utf-8: {error}"))
        })?;
        let byte = u8::from_str_radix(chunk, 16).map_err(|error| {
            AppError::backend(format!("failed to parse hex byte `{chunk}`: {error}"))
        })?;
        output.push(byte);
    }

    Ok(output)
}

fn pick_free_port() -> AppResult<u16> {
    let listener = TcpListener::bind((DEFAULT_HOST, 0)).map_err(|source| {
        AppError::backend(format!("failed to allocate local port for anvil: {source}"))
    })?;
    let port = listener
        .local_addr()
        .map_err(|source| AppError::backend(format!("failed to inspect local listener: {source}")))?
        .port();
    drop(listener);
    Ok(port)
}

fn anvil_binary() -> PathBuf {
    std::env::var_os("ANVIL_BIN")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("anvil"))
}

#[allow(dead_code)]
fn _io_debug(_: io::Error) {}
