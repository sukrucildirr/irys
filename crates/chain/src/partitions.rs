use std::{ops::Index, sync::mpsc::Receiver};

use actix::{Actor, Addr, Context, Handler, Message};
use irys_storage::{ii, partition_provider::PartitionStorageProvider};
use irys_types::{
    block_production::{Partition, SolutionContext},
    PartitionStorageProviderConfig, StorageModuleConfig, CHUNK_SIZE, H256,
    NUM_CHUNKS_IN_RECALL_RANGE, NUM_OF_CHUNKS_IN_PARTITION, NUM_RECALL_RANGES_IN_PARTITION, U256,
};
use rand::{seq::SliceRandom, RngCore, SeedableRng};
use rand_chacha::ChaCha20Rng;
use sha2::{Digest, Sha256};

pub fn get_partitions_and_storage_providers(
) -> eyre::Result<Vec<(Partition, PartitionStorageProvider)>> {
    Ok(vec![
        (
            Partition::default(),
            PartitionStorageProvider::from_config(PartitionStorageProviderConfig {
                sm_paths_offsets: vec![
                    (
                        ii(0, 3),
                        StorageModuleConfig {
                            directory_path: "/tmp/sm/sm1".into(),
                            size_bytes: 10 * CHUNK_SIZE,
                        },
                    ),
                    (
                        ii(4, 10),
                        StorageModuleConfig {
                            directory_path: "/tmp/sm/sm2".into(),
                            size_bytes: 10 * CHUNK_SIZE,
                        },
                    ),
                ],
            })?,
        ),
        (
            Partition::default(),
            PartitionStorageProvider::from_config(PartitionStorageProviderConfig {
                sm_paths_offsets: vec![
                    (
                        ii(0, 5),
                        StorageModuleConfig {
                            directory_path: "/tmp/sm/sm3".into(),
                            size_bytes: 10 * CHUNK_SIZE,
                        },
                    ),
                    (
                        ii(6, 10),
                        StorageModuleConfig {
                            directory_path: "/tmp/sm/sm4".into(),
                            size_bytes: 10 * CHUNK_SIZE,
                        },
                    ),
                ],
            })?,
        ),
    ])
}

pub fn mine_partition(partition: Partition, seed_receiver_channel: Receiver<H256>) {
    // Random difficulty
    let mut difficulty = U256::from_little_endian(H256::random().as_bytes());
    dbg!("Difficulty at start is {}", difficulty);
    loop {
        let mining_hash = match seed_receiver_channel.recv() {
            Ok(h) => h,
            Err(e) => panic!("Mine partition channel dead {}", e),
        };

        // Startup message to ignore
        if mining_hash == H256::zero() {
            continue;
        }

        // TODO: add a partition_state that keeps track of efficient sampling
        let mut rng = ChaCha20Rng::from_seed(mining_hash.into());

        // For now, Pick a random recall range in the partition
        let recall_range_index = rng.next_u64() % NUM_RECALL_RANGES_IN_PARTITION;

        // Starting chunk index within partition
        let start_chunk_index = (recall_range_index * NUM_CHUNKS_IN_RECALL_RANGE) as usize;

        // Create a contiguous piece of memory on the heap where chunks can be written into
        let mut chunks_buffer: Vec<[u8; CHUNK_SIZE as usize]> =
            Vec::with_capacity((NUM_CHUNKS_IN_RECALL_RANGE * CHUNK_SIZE) as usize);

        // TODO: read chunks. For now creates random
        for _ in 0..NUM_CHUNKS_IN_RECALL_RANGE {
            let mut data = [0u8; CHUNK_SIZE as usize];
            rand::thread_rng().fill_bytes(&mut data);
            chunks_buffer.push(data);
        }

        let mut hasher = Sha256::new();
        for i in 0..NUM_CHUNKS_IN_RECALL_RANGE {
            let chunk: &[u8] = &chunks_buffer[i as usize];

            hasher.update(chunk);
            let hash = hasher.finalize_reset().to_vec();

            // TODO: check if difficulty higher now. Will look in DB for latest difficulty info and update difficulty

            let solution_number = hash_to_number(&hash);
            if solution_number >= U256::from(difficulty) {
                dbg!("SOLUTION FOUND!!!!!!!!!");
                let solution = SolutionContext {
                    partition_id: partition.id,
                    // TODO: Fix
                    chunk_index: 0,
                    mining_address: partition.mining_address,
                };
                // TODO: Send info to block builder code

                // TODO: Let all partitions know to stop mining

                // Once solution is sent stop mining and let all other partitions know
                break;
            }
        }
    }
}

fn hash_to_number(hash: &[u8]) -> U256 {
    U256::from_little_endian(hash)
}
