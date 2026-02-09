use crate::{
    config::HistorySourceConfig,
    error::AppResult,
    history::{jsonl::JsonlSource, rpc_range::RpcRangeSource},
    types::BlockSwaps,
};

pub trait SwapInputSource {
    fn next_block(&mut self) -> AppResult<Option<BlockSwaps>>;

    fn collect_all(&mut self) -> AppResult<Vec<BlockSwaps>> {
        let mut blocks = Vec::new();

        while let Some(block) = self.next_block()? {
            blocks.push(block);
        }

        Ok(blocks)
    }
}

#[derive(Debug)]
pub enum HistoryInputSource {
    Jsonl(JsonlSource),
    RpcRange(RpcRangeSource),
}

pub fn build_history_source(config: &HistorySourceConfig) -> AppResult<HistoryInputSource> {
    match config {
        HistorySourceConfig::Jsonl { .. } => {
            Ok(HistoryInputSource::Jsonl(JsonlSource::from_config(config)?))
        }
        HistorySourceConfig::RpcRange { .. } => Ok(HistoryInputSource::RpcRange(
            RpcRangeSource::from_config(config)?,
        )),
    }
}

impl SwapInputSource for HistoryInputSource {
    fn next_block(&mut self) -> AppResult<Option<BlockSwaps>> {
        match self {
            Self::Jsonl(source) => source.next_block(),
            Self::RpcRange(source) => source.next_block(),
        }
    }
}
