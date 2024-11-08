use actix::Message;
use alloy_primitives::Address;

use crate::H256;

#[derive(Message)]
#[rtype(result = "()")]
pub struct SolutionContext {
    pub partition_id: u64,
    pub chunk_index: u64,
    pub mining_address: Address,
}

pub struct Partition {
    pub id: PartitionId,
    pub mining_address: Address,
}

pub type PartitionId = u64;

impl Default for Partition {
    fn default() -> Self {
        Self {
            id: 0,
            mining_address: Address::random(),
        }
    }
}
