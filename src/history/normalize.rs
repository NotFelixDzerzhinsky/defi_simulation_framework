use std::collections::BTreeMap;

use crate::types::{BlockSwaps, SwapRequest};

pub fn group_swaps_by_block(swaps: Vec<SwapRequest>) -> Vec<BlockSwaps> {
    let mut grouped: BTreeMap<u64, Vec<SwapRequest>> = BTreeMap::new();

    for swap in swaps {
        grouped.entry(swap.block_number).or_default().push(swap);
    }

    grouped
        .into_iter()
        .map(|(block_number, swaps)| BlockSwaps {
            block_number,
            swaps,
        })
        .collect()
}
