use alloy_core::primitives::keccak256;
use irys_types::{
    merkle::{generate_data_root, generate_leaves, resolve_proofs},
    Address, Base64, IrysSignature, IrysTransaction, IrysTransactionHeader, Signature, H256,
    IRYS_CHAIN_ID,
};

use eyre::Result;
use k256::ecdsa::SigningKey;
use rand::rngs::OsRng;

pub struct Irys {
    pub signer: SigningKey,
    pub chain_id: u64,
}

impl Irys {
    pub fn random_signer() -> Self {
        Irys {
            signer: k256::ecdsa::SigningKey::random(&mut OsRng),
            chain_id: IRYS_CHAIN_ID,
        }
    }

    /// Creates a transaction from a data buffer, optional anchor hash for the
    /// transaction is supported. The txid will not be set until the transaction
    /// is signed with [sign_transaction]
    pub async fn create_transaction(
        &self,
        data: Vec<u8>,
        anchor: Option<H256>, //TODO!: more parameters as they are implemented
    ) -> Result<IrysTransaction> {
        let mut transaction = self.merklize(data)?;

        // TODO: These should be calculated from some pricing params passed in
        // as a parameter
        transaction.header.perm_fee = Some(1);
        transaction.header.term_fee = 1;

        // Fetch and set last_tx if not provided (primarily for testing).
        let anchor = if let Some(anchor) = anchor {
            anchor
        } else {
            // TODO: Retrieve an acceptable block_hash anchor
            H256::default()
        };
        transaction.header.anchor = anchor;

        Ok(transaction)
    }

    /// signs and sets signature and id.
    pub fn sign_transaction(&self, mut transaction: IrysTransaction) -> Result<IrysTransaction> {
        // Store the signer address
        transaction.header.signer = Address::from_public_key(self.signer.verifying_key());

        // Create the signature hash and sign it
        let prehash = transaction.signature_hash();
        let mut signature: Signature = self.signer.sign_prehash_recoverable(&prehash)?.into();

        transaction.header.signature.reth_signature = signature.with_chain_id(self.chain_id);

        // Drives the the txid by hashing the signature
        let id: [u8; 32] = keccak256(signature.as_bytes()).into();
        transaction.header.id = H256::from(id);
        Ok(transaction)
    }

    /// Builds a merkle tree, with a root, including all the proofs for each
    /// chunk.
    fn merklize(&self, data: Vec<u8>) -> Result<IrysTransaction> {
        let mut chunks = generate_leaves(data.clone())?;
        let root = generate_data_root(chunks.clone())?;
        let data_root = H256(root.id.clone());
        let mut proofs = resolve_proofs(root, None)?;

        // Discard the last chunk & proof if it's zero length.
        let last_chunk = chunks.last().unwrap();
        if last_chunk.max_byte_range == last_chunk.min_byte_range {
            chunks.pop();
            proofs.pop();
        }

        Ok(IrysTransaction {
            header: IrysTransactionHeader {
                data_size: data.len() as u64,
                data_root,
                ..Default::default()
            },
            data: Base64(data),
            chunks,
            proofs,
            ..Default::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use alloy_core::primitives::keccak256;
    use assert_matches::assert_matches;
    use irys_types::{
        hash_sha256,
        merkle::{validate_chunk, MAX_CHUNK_SIZE},
        Compact,
    };
    use rand::Rng;
    use reth_primitives::recover_signer_unchecked;

    use super::Irys;

    #[tokio::test]
    async fn create_and_sign_transaction() {
        // Create 2.5 chunks worth of data *  fill the data with random bytes
        let data_size = (MAX_CHUNK_SIZE as f64 * 2.5).round() as usize;
        let mut data_bytes = vec![0u8; data_size];
        rand::thread_rng().fill(&mut data_bytes[..]);

        // Create a new Irys API instance
        let irys = Irys::random_signer();

        // Create a transaction from the random bytes
        let mut tx = irys
            .create_transaction(data_bytes.clone(), None)
            .await
            .unwrap();

        // Sign the transaction
        tx = irys.sign_transaction(tx).unwrap();

        assert_eq!(tx.chunks.len(), 3);

        for chunk in &tx.chunks {
            println!(
                "min: {}, max: {}",
                chunk.min_byte_range, chunk.max_byte_range
            );
        }

        print!("{}\n", serde_json::to_string_pretty(&tx.header).unwrap());

        // Make sure the size of the last chunk is just whatever is left over
        // after chunking the rest of the data at MAX_CHUNK_SIZE intervals.
        let last_chunk = tx.chunks.last().unwrap();
        assert_eq!(
            data_size % MAX_CHUNK_SIZE,
            last_chunk.max_byte_range - last_chunk.min_byte_range
        );

        // Validate the chunk proofs
        for (index, chunk) in tx.chunks.iter().enumerate() {
            let min = chunk.min_byte_range;
            let max = chunk.max_byte_range;

            // Ensure max is within bounds of data_bytes
            if max > data_bytes.len() {
                panic!("Max byte range exceeds the data_bytes length!");
            }

            // Ensure every chunk proof (data_path) is valid
            let proof_result = validate_chunk(
                tx.header.data_root.0,
                chunk.clone(),
                tx.proofs[index].clone(),
            );
            assert_matches!(proof_result, Ok(_));

            // Ensure the data_hash is valid by hashing the chunk data
            let chunk_bytes: &[u8] = &data_bytes[min..max];
            let computed_hash = hash_sha256(&chunk_bytes).unwrap();
            let data_hash = chunk.data_hash.unwrap();

            assert_eq!(data_hash, computed_hash);
        }

        // Recover the signer as a way to verify the signature
        let prehash = tx.header.signature_hash();
        let sig = tx.header.signature.as_bytes();
        let signer = recover_signer_unchecked(&sig, &prehash).ok();

        assert_eq!(signer.unwrap(), tx.header.signer);
    }
}
