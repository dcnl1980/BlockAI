use crate::ShardError;
use blockai_types::{
    receipt_leaf_hash, AmountMicros, PaymentProof,
};

#[derive(Clone, Debug, Default)]
pub struct ReceiptLog {
    proofs: Vec<PaymentProof>,
    leaves: Vec<[u8; 32]>,
    exposure: AmountMicros,
}

impl ReceiptLog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.proofs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.proofs.is_empty()
    }

    pub fn exposure(&self) -> AmountMicros {
        self.exposure
    }

    pub fn leaves(&self) -> &[[u8; 32]] {
        &self.leaves
    }

    pub fn proofs(&self) -> &[PaymentProof] {
        &self.proofs
    }

    pub fn append(&mut self, proof: PaymentProof) -> Result<(), ShardError> {
        let leaf = receipt_leaf_hash(&proof).map_err(|_| ShardError::Cbor)?;
        self.exposure = AmountMicros(self.exposure.0 + proof.service.actual_amount.0);
        self.leaves.push(leaf);
        self.proofs.push(proof);
        Ok(())
    }

    pub fn clear_sealed(&mut self) {
        self.proofs.clear();
        self.leaves.clear();
        self.exposure = AmountMicros(0);
    }
}
