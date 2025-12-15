//! Pending multi-signature transaction handling
//!
//! Manages transactions awaiting signatures from multiple parties.

use crate::core::{Transaction, TransactionBuilder, TransactionOutput, UTXO};
use crate::crypto::{public_key_from_hex, sha256, verify_signature, KeyPair};
use crate::multisig::wallet::{MultisigError, MultisigWallet};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A single signature from a multisig participant
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MultisigSignature {
    /// Public key of the signer (hex)
    pub signer_pubkey: String,
    /// Signature over the transaction (hex)
    pub signature: String,
    /// When the signature was added
    pub signed_at: DateTime<Utc>,
}

impl MultisigSignature {
    /// Create a new signature
    pub fn new(signer_pubkey: String, signature: String) -> Self {
        Self {
            signer_pubkey,
            signature,
            signed_at: Utc::now(),
        }
    }

    /// Verify this signature against a message hash
    pub fn verify(&self, message_hash: &[u8]) -> Result<bool, MultisigError> {
        let pubkey = public_key_from_hex(&self.signer_pubkey)?;
        let sig_bytes =
            hex::decode(&self.signature).map_err(|_| MultisigError::InvalidSignature)?;
        Ok(verify_signature(&pubkey, message_hash, &sig_bytes)?)
    }
}

/// Status of a pending multisig transaction
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum PendingStatus {
    /// Waiting for more signatures
    AwaitingSignatures,
    /// Has enough signatures, ready to broadcast
    Ready,
    /// Transaction has been broadcast to the network
    Broadcast,
    /// Transaction expired or was cancelled
    Expired,
}

/// A transaction pending signature collection
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PendingMultisigTx {
    /// Unique transaction ID
    pub id: String,
    /// Source multisig wallet address
    pub from_address: String,
    /// Recipient address
    pub to_address: String,
    /// Amount to send
    pub amount: u64,
    /// UTXOs being spent
    pub input_utxos: Vec<UTXO>,
    /// Transaction outputs (including change)
    pub outputs: Vec<TransactionOutput>,
    /// Collected signatures
    pub signatures: Vec<MultisigSignature>,
    /// Required signature threshold
    pub threshold: u8,
    /// Current status
    pub status: PendingStatus,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// When status last changed
    pub updated_at: DateTime<Utc>,
}

impl PendingMultisigTx {
    /// Create a new pending transaction
    pub fn new(
        from_address: String,
        to_address: String,
        amount: u64,
        input_utxos: Vec<UTXO>,
        outputs: Vec<TransactionOutput>,
        threshold: u8,
    ) -> Self {
        let now = Utc::now();

        // Generate unique ID from transaction details
        let id_data = format!(
            "{}{}{}{}{}",
            from_address,
            to_address,
            amount,
            now.timestamp_nanos_opt().unwrap_or(0),
            input_utxos.len()
        );
        let id = hex::encode(&sha256(id_data.as_bytes())[..16]);

        Self {
            id,
            from_address,
            to_address,
            amount,
            input_utxos,
            outputs,
            signatures: Vec::new(),
            threshold,
            status: PendingStatus::AwaitingSignatures,
            created_at: now,
            updated_at: now,
        }
    }

    /// Get the data that needs to be signed
    pub fn signing_data(&self) -> Vec<u8> {
        // Hash outputs + from + to + amount for signing
        let data = format!(
            "{:?}{}{}{}",
            self.outputs, self.from_address, self.to_address, self.amount
        );
        sha256(data.as_bytes())
    }

    /// Add a signature from an authorized signer
    pub fn add_signature(
        &mut self,
        signature: MultisigSignature,
        wallet: &MultisigWallet,
    ) -> Result<(), MultisigError> {
        // Check signer is authorized
        if !wallet.is_signer(&signature.signer_pubkey) {
            return Err(MultisigError::UnauthorizedSigner(
                signature.signer_pubkey.clone(),
            ));
        }

        // Check not already signed by this signer
        if self
            .signatures
            .iter()
            .any(|s| s.signer_pubkey == signature.signer_pubkey)
        {
            return Err(MultisigError::AlreadySigned);
        }

        // Verify the signature
        let signing_data = self.signing_data();
        if !signature.verify(&signing_data)? {
            return Err(MultisigError::InvalidSignature);
        }

        // Add signature
        self.signatures.push(signature);
        self.updated_at = Utc::now();

        // Check if we have enough signatures
        if self.signatures.len() >= self.threshold as usize {
            self.status = PendingStatus::Ready;
        }

        Ok(())
    }

    /// Get number of signatures collected
    pub fn signature_count(&self) -> usize {
        self.signatures.len()
    }

    /// Check if transaction has enough signatures
    pub fn is_ready(&self) -> bool {
        self.status == PendingStatus::Ready || self.signatures.len() >= self.threshold as usize
    }

    /// Get signers who have already signed
    pub fn signed_by(&self) -> Vec<&str> {
        self.signatures
            .iter()
            .map(|s| s.signer_pubkey.as_str())
            .collect()
    }

    /// Build the final signed transaction
    ///
    /// This creates a standard transaction with all collected signatures
    /// concatenated in the signature field.
    pub fn finalize(&self) -> Result<Transaction, MultisigError> {
        if !self.is_ready() {
            return Err(MultisigError::InsufficientSignatures {
                have: self.signatures.len(),
                need: self.threshold,
            });
        }

        // Build transaction from UTXOs and outputs
        let mut builder = TransactionBuilder::new();

        for utxo in &self.input_utxos {
            builder = builder.add_input(utxo);
        }

        for output in &self.outputs {
            builder = builder.add_output(&output.recipient, output.amount);
        }

        let mut tx = builder.build();

        // Combine all signatures and public keys
        // Format: comma-separated "pubkey:signature" pairs
        let combined_sigs: Vec<String> = self
            .signatures
            .iter()
            .map(|s| format!("{}:{}", s.signer_pubkey, s.signature))
            .collect();
        let multisig_data = combined_sigs.join(",");

        // Set the combined signature on all inputs
        for input in &mut tx.inputs {
            input.signature = multisig_data.clone();
            input.public_key = format!("MULTISIG:{}", self.from_address);
        }

        // Recalculate hash
        tx.id = tx.calculate_hash();

        Ok(tx)
    }

    /// Mark as broadcast
    pub fn mark_broadcast(&mut self) {
        self.status = PendingStatus::Broadcast;
        self.updated_at = Utc::now();
    }

    /// Mark as expired
    pub fn mark_expired(&mut self) {
        self.status = PendingStatus::Expired;
        self.updated_at = Utc::now();
    }
}

/// Helper to create a signature for a pending transaction
pub fn sign_pending_tx(
    pending: &PendingMultisigTx,
    key_pair: &KeyPair,
) -> Result<MultisigSignature, MultisigError> {
    let signing_data = pending.signing_data();
    let signature = key_pair.sign(&signing_data)?;

    Ok(MultisigSignature::new(
        key_pair.public_key_hex(),
        hex::encode(&signature),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::TransactionOutput;
    use crate::crypto::KeyPair;
    use crate::multisig::MultisigConfig;

    fn create_test_wallet() -> (MultisigWallet, Vec<KeyPair>) {
        let keys: Vec<KeyPair> = (0..3).map(|_| KeyPair::generate()).collect();
        let pubkeys: Vec<String> = keys.iter().map(|k| k.public_key_hex()).collect();

        let config = MultisigConfig::new(2, pubkeys, Some("Test".to_string())).unwrap();
        let wallet = MultisigWallet::new(config).unwrap();

        (wallet, keys)
    }

    #[test]
    fn test_pending_tx_creation() {
        let (wallet, _) = create_test_wallet();

        let outputs = vec![TransactionOutput {
            amount: 50,
            recipient: "recipient".to_string(),
        }];

        let pending = PendingMultisigTx::new(
            wallet.address().to_string(),
            "recipient".to_string(),
            50,
            vec![],
            outputs,
            wallet.threshold(),
        );

        assert_eq!(pending.threshold, 2);
        assert_eq!(pending.signature_count(), 0);
        assert!(!pending.is_ready());
        assert_eq!(pending.status, PendingStatus::AwaitingSignatures);
    }

    #[test]
    fn test_signature_collection() {
        let (wallet, keys) = create_test_wallet();

        let outputs = vec![TransactionOutput {
            amount: 50,
            recipient: "recipient".to_string(),
        }];

        let mut pending = PendingMultisigTx::new(
            wallet.address().to_string(),
            "recipient".to_string(),
            50,
            vec![],
            outputs,
            wallet.threshold(),
        );

        // Sign with first key
        let sig1 = sign_pending_tx(&pending, &keys[0]).unwrap();
        pending.add_signature(sig1, &wallet).unwrap();
        assert_eq!(pending.signature_count(), 1);
        assert!(!pending.is_ready());

        // Sign with second key
        let sig2 = sign_pending_tx(&pending, &keys[1]).unwrap();
        pending.add_signature(sig2, &wallet).unwrap();
        assert_eq!(pending.signature_count(), 2);
        assert!(pending.is_ready());
        assert_eq!(pending.status, PendingStatus::Ready);
    }

    #[test]
    fn test_duplicate_signature_rejected() {
        let (wallet, keys) = create_test_wallet();

        let outputs = vec![TransactionOutput {
            amount: 50,
            recipient: "recipient".to_string(),
        }];

        let mut pending = PendingMultisigTx::new(
            wallet.address().to_string(),
            "recipient".to_string(),
            50,
            vec![],
            outputs,
            wallet.threshold(),
        );

        // Sign with first key
        let sig1 = sign_pending_tx(&pending, &keys[0]).unwrap();
        pending.add_signature(sig1, &wallet).unwrap();

        // Try to sign again with same key
        let sig1_again = sign_pending_tx(&pending, &keys[0]).unwrap();
        let result = pending.add_signature(sig1_again, &wallet);
        assert!(matches!(result, Err(MultisigError::AlreadySigned)));
    }

    #[test]
    fn test_unauthorized_signer_rejected() {
        let (wallet, _) = create_test_wallet();
        let unauthorized_key = KeyPair::generate();

        let outputs = vec![TransactionOutput {
            amount: 50,
            recipient: "recipient".to_string(),
        }];

        let mut pending = PendingMultisigTx::new(
            wallet.address().to_string(),
            "recipient".to_string(),
            50,
            vec![],
            outputs,
            wallet.threshold(),
        );

        // Try to sign with unauthorized key
        let sig = sign_pending_tx(&pending, &unauthorized_key).unwrap();
        let result = pending.add_signature(sig, &wallet);
        assert!(matches!(result, Err(MultisigError::UnauthorizedSigner(_))));
    }

    #[test]
    fn test_finalize_transaction() {
        let (wallet, keys) = create_test_wallet();

        let utxo = UTXO {
            tx_id: "prev_tx".to_string(),
            output_index: 0,
            output: TransactionOutput {
                amount: 100,
                recipient: wallet.address().to_string(),
            },
        };

        let outputs = vec![
            TransactionOutput {
                amount: 50,
                recipient: "recipient".to_string(),
            },
            TransactionOutput {
                amount: 50,
                recipient: wallet.address().to_string(),
            },
        ];

        let mut pending = PendingMultisigTx::new(
            wallet.address().to_string(),
            "recipient".to_string(),
            50,
            vec![utxo],
            outputs,
            wallet.threshold(),
        );

        // Collect signatures
        let sig1 = sign_pending_tx(&pending, &keys[0]).unwrap();
        let sig2 = sign_pending_tx(&pending, &keys[1]).unwrap();
        pending.add_signature(sig1, &wallet).unwrap();
        pending.add_signature(sig2, &wallet).unwrap();

        // Finalize
        let tx = pending.finalize().unwrap();
        assert!(!tx.id.is_empty());
        assert_eq!(tx.outputs.len(), 2);
        assert!(tx.inputs[0].public_key.starts_with("MULTISIG:"));
    }
}
