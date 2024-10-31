pub const BLOCK_TIME: u64 = 30_000;

pub const CHUNK_SIZE: u64 = 256 * 1024;

pub const NUM_OF_CHUNKS_IN_PARTITION: u64 = 10;

pub const PARTITION_SIZE: u64 = CHUNK_SIZE * NUM_OF_CHUNKS_IN_PARTITION;

pub const NUM_CHUNKS_IN_RECALL_RANGE: u64 = 2;

pub const NUM_RECALL_RANGES_IN_PARTITION: u64 =
    NUM_OF_CHUNKS_IN_PARTITION / NUM_CHUNKS_IN_RECALL_RANGE;

// Reset the nonce limiter (vdf) once every 1200 steps/seconds or every ~20 min
pub const NONCE_LIMITER_RESET_FREQUENCY: usize = 10 * 120;

// 25 checkpoints 40 ms each = 1000 ms
pub const NUM_CHECKPOINTS_IN_VDF_STEP: usize = 25;

// Typical ryzen 5900X iterations for 1 sec
// pub const VDF_SHA_1S: u64 = 15_000_000;
pub const VDF_SHA_1S: u64 = 100_000;

pub const HASHES_PER_CHECKPOINT: u64 = VDF_SHA_1S / NUM_CHECKPOINTS_IN_VDF_STEP as u64;

pub const IRYS_CHAIN_ID: u64 = 69727973; // "irys" in ascii

// Epoch and capacity projection parameters
pub const NUM_REPLICAS_PER_LEDGER_INDEX: u64 = 1;
pub const CAPACITY_SCALAR: u64 = 100; // Scaling factor for the capacity projection curve
