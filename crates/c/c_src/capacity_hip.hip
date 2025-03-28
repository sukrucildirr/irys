#include <openssl/evp.h>
#include <string.h>
#include <stdlib.h>
#include <stdint.h>
#include "hip/hip_runtime.h"

#include "erl_nif.h"

#include "capacity.h"
#include "sha256.cuh"

// Max threads - this is the maximum number of threads per block for the GPU
// This is a constant defined by the GPU architecture.
#define THREADS_PER_BLOCK 512
#define NUM_HASHES (DATA_CHUNK_SIZE / PACKING_HASH_SIZE)
#define INPUT_SIZE PACKING_HASH_SIZE

#define SHA256_DIGEST_LENGTH   PACKING_HASH_SIZE

/**
 * Computes the seed hash for the given chunk.
 * @param chunk_id The chunk id
 * @param chunk_id_length The length of the chunk id
 * @param seed_hash The output seed hash
 */
__device__ void compute_seed_hash_hip(const unsigned char *chunk_id, size_t chunk_id_length, unsigned char *seed_hash) {
    SHA256_CTX sha256;
    sha256_init(&sha256);
    sha256_update(&sha256, chunk_id, chunk_id_length);
    sha256_final(&sha256, seed_hash);
}

/**
 * Computes the start entropy chunk for the given chunk.
 * @param previous_segment The previous segment
 * @param previous_segment_len The length of the previous segment
 * @param chunk The output chunk
 */
__device__ void compute_start_entropy_chunk2_hip(const unsigned char *previous_segment, size_t previous_segment_len, unsigned char *chunk) {
    size_t chunk_len = 0;
    SHA256_CTX sha256;

    while (chunk_len < DATA_CHUNK_SIZE) {
        sha256_init(&sha256);
        sha256_update(&sha256, previous_segment, SHA256_DIGEST_LENGTH);
        sha256_final(&sha256, chunk + chunk_len);

        previous_segment = chunk + chunk_len;
        chunk_len += SHA256_DIGEST_LENGTH;
    }
}

/**
 * Computes the start entropy chunk for the given chunk.
 * @param chunk_id The chunk id
 * @param chunk_id_length The length of the chunk id
 * @param chunk The output chunk
 */
__device__ void compute_start_entropy_chunk_hip(const unsigned char *chunk_id, size_t chunk_id_length, unsigned char *chunk) {
    unsigned char seed_hash[PACKING_HASH_SIZE];

    compute_seed_hash_hip(chunk_id, chunk_id_length, seed_hash);
    compute_start_entropy_chunk2_hip(seed_hash, PACKING_HASH_SIZE, chunk);
}

/**
 * Computes the entropy chunk for the given chunk.
 * @param segment The segment
 * @param entropy_chunk The entropy chunk
 * @param new_entropy_chunk The new entropy chunk
 * @param packing_sha_1_5_s The number of iterations
 */
__device__ void compute_entropy_chunk2_hip(const unsigned char *segment, const unsigned char *entropy_chunk, unsigned char *new_entropy_chunk, unsigned int packing_sha_1_5_s) {
    memcpy(new_entropy_chunk, entropy_chunk, DATA_CHUNK_SIZE);
    SHA256_CTX sha256;

    for (int hash_count = HASH_ITERATIONS_PER_BLOCK; hash_count < packing_sha_1_5_s; ++hash_count) {
        size_t start_offset = (hash_count % HASH_ITERATIONS_PER_BLOCK) * PACKING_HASH_SIZE;

        sha256_init(&sha256);
        sha256_update(&sha256, segment, SHA256_DIGEST_LENGTH);
        sha256_update(&sha256, entropy_chunk + start_offset, SHA256_DIGEST_LENGTH);
        sha256_final(&sha256, new_entropy_chunk + start_offset);
        segment = new_entropy_chunk + start_offset;
    }
}

/**
 * Computes the entropy chunk for the given chunk.
 * @param chunk_id The chunk id
 * @param chunk_id_length The length of the chunk id
 * @param entropy_chunk The entropy chunk
 * @param chunk_1 First layer of the chunk
 * @param packing_sha_1_5_s The number of iterations
 */
__device__ void compute_entropy_chunk_hip(const unsigned char *chunk_id, size_t chunk_id_length, unsigned char *entropy_chunk, unsigned char *chunk_1, unsigned int packing_sha_1_5_s) {
    const int partial_entropy_chunk_size = (HASH_ITERATIONS_PER_BLOCK - 1) * PACKING_HASH_SIZE;
    unsigned char *start_entropy_chunk = chunk_1;

    compute_start_entropy_chunk_hip(chunk_id, chunk_id_length, start_entropy_chunk);

    unsigned char last_entropy_chunk_segment[PACKING_HASH_SIZE];
    memcpy(last_entropy_chunk_segment, start_entropy_chunk + partial_entropy_chunk_size, PACKING_HASH_SIZE);

    compute_entropy_chunk2_hip(last_entropy_chunk_segment, start_entropy_chunk, entropy_chunk, packing_sha_1_5_s);
}

/**
 * Computes the entropy chunks for the given list of chunks.
 * The entropy chunks are computed in parallel using the GPU.
 */
__global__ void compute_entropy_chunks_hip_kernel(unsigned char *chunk_id, unsigned long int chunk_offset_start, long int chunks_count, unsigned char *chunks, unsigned char *chunks_1, unsigned int packing_sha_1_5_s) {
    // Get the index of the current thread - as we are using a 1D grid, we only need to get the index of the current block.
    // The index of the current thread is then the index of the block times the number of threads per block plus the index of the current thread in the block.
    int idx = blockIdx.x * blockDim.x + threadIdx.x;
    if (idx < chunks_count) {
        unsigned char *output = chunks + idx * DATA_CHUNK_SIZE;
        unsigned char *chunk_1 = chunks_1 + idx * DATA_CHUNK_SIZE;
        unsigned char chunk_id_thread[CHUNK_ID_LEN];
        memcpy(chunk_id_thread, chunk_id, CHUNK_ID_LEN - sizeof(uint64_t));
        *((uint32_t*)&chunk_id_thread[CHUNK_ID_LEN - sizeof(uint64_t)]) = chunk_offset_start + idx * DATA_CHUNK_SIZE;
        compute_entropy_chunk_hip(chunk_id_thread, CHUNK_ID_LEN, output, chunk_1, packing_sha_1_5_s);
    }
}

/**
 * Computes the entropy chunks for the given list of chunks.
 * The entropy chunks are computed in parallel using the GPU.
 */
extern "C" entropy_chunk_errors compute_entropy_chunks_hip(const unsigned char *mining_addr, size_t mining_addr_size, unsigned long int chunk_offset_start, long int chunks_count, const unsigned char *partition_hash, size_t partition_hash_size, unsigned char *chunks, unsigned int packing_sha_1_5_s)
{
#ifdef BENCHMARK_PARALLEL
    printf("HIP: Entropy chunks computation started\n");
#endif
    unsigned char *d_chunks;
    unsigned char *d_chunks_1;
    unsigned char *d_chunk_id;

    // We need to allocate memory for each layer here, as kernel/device functions can't allocate any
    // additional memory, so we need to prepare it for them.
    if (hipMalloc(&d_chunks, DATA_CHUNK_SIZE * chunks_count) != hipSuccess) {
        return HIP_ERROR;
    }

    if (hipMalloc(&d_chunks_1, DATA_CHUNK_SIZE * chunks_count) != hipSuccess) {
        return HIP_ERROR;
    }

    if (hipMalloc(&d_chunk_id, CHUNK_ID_LEN) != hipSuccess) {
        return HIP_ERROR;
    }

    if (hipMemcpy(d_chunk_id, mining_addr, mining_addr_size, hipMemcpyHostToDevice) != hipSuccess) {
        return HIP_ERROR;
    }

    if (hipMemcpy(d_chunk_id + mining_addr_size, partition_hash, partition_hash_size, hipMemcpyHostToDevice) != hipSuccess) {
        return HIP_ERROR;
    }

    // Launch kernel
    int blocks = (chunks_count + THREADS_PER_BLOCK - 1) / THREADS_PER_BLOCK;
    compute_entropy_chunks_hip_kernel<<<blocks, THREADS_PER_BLOCK>>>(d_chunk_id, chunk_offset_start, chunks_count, d_chunks, d_chunks_1, packing_sha_1_5_s);

    // Check for errors after kernel launch
    hipError_t err = hipGetLastError();
    if (err != hipSuccess) {
        return HIP_KERNEL_LAUNCH_FAILED;
    }

    hipError_t errm = hipMemcpy(chunks, d_chunks, DATA_CHUNK_SIZE * chunks_count, hipMemcpyDeviceToHost);
    if (errm != hipSuccess) {
        return HIP_ERROR;
    }

    errm = hipFree(d_chunks);
    if (errm != hipSuccess) {
        return HIP_ERROR;
    }

    errm = hipFree(d_chunks_1);
    if (errm != hipSuccess) {
        return HIP_ERROR;
    }

    errm = hipFree(d_chunk_id);
    if (errm != hipSuccess) {
        return HIP_ERROR;
    }

    return NO_ERROR;
}