//! Script System for output locking conditions
//!
//! Implements Bitcoin-like script types for transaction outputs.
//! This is a simplified version focused on the most common patterns.

use serde::{Deserialize, Serialize};
use thiserror::Error;

// =============================================================================
// Script Constants
// =============================================================================

/// Script version for future upgrades
pub const SCRIPT_VERSION: u8 = 1;

// =============================================================================
// Script Errors
// =============================================================================

/// Script-related errors
#[derive(Error, Debug, Clone)]
pub enum ScriptError {
    #[error("Invalid script type")]
    InvalidScriptType,
    #[error("Script execution failed: {0}")]
    ExecutionFailed(String),
    #[error("Invalid public key")]
    InvalidPublicKey,
    #[error("Invalid signature")]
    InvalidSignature,
    #[error("Threshold not met: need {0} of {1} signatures, got {2}")]
    ThresholdNotMet(u8, u8, u8),
    #[error("Timelock not expired: {0}")]
    TimelockActive(u32),
    #[error("Script too large: {0} bytes")]
    ScriptTooLarge(usize),
}

// =============================================================================
// Signature Hash Types (Bitcoin BIP-143)
// =============================================================================

/// Signature hash type determines what parts of the transaction are signed
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum SigHashType {
    /// Sign all inputs and all outputs (default)
    All = 0x01,
    /// Sign all inputs but no outputs (blank check)
    None = 0x02,
    /// Sign all inputs and only the output with same index
    Single = 0x03,
    /// Only sign own input (can be combined with above)
    AnyoneCanPay = 0x80,
    /// SIGHASH_ALL | SIGHASH_ANYONECANPAY
    AllAnyoneCanPay = 0x81,
    /// SIGHASH_NONE | SIGHASH_ANYONECANPAY
    NoneAnyoneCanPay = 0x82,
    /// SIGHASH_SINGLE | SIGHASH_ANYONECANPAY
    SingleAnyoneCanPay = 0x83,
}

impl Default for SigHashType {
    fn default() -> Self {
        SigHashType::All
    }
}

impl SigHashType {
    /// Parse sighash type from byte
    pub fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            0x01 => Some(SigHashType::All),
            0x02 => Some(SigHashType::None),
            0x03 => Some(SigHashType::Single),
            0x80 => Some(SigHashType::AnyoneCanPay),
            0x81 => Some(SigHashType::AllAnyoneCanPay),
            0x82 => Some(SigHashType::NoneAnyoneCanPay),
            0x83 => Some(SigHashType::SingleAnyoneCanPay),
            _ => None,
        }
    }

    /// Check if this sighash includes ANYONECANPAY flag
    pub fn is_anyone_can_pay(&self) -> bool {
        (*self as u8) & 0x80 != 0
    }

    /// Get the base type (without ANYONECANPAY flag)
    pub fn base_type(&self) -> SigHashType {
        match (*self as u8) & 0x1f {
            0x01 => SigHashType::All,
            0x02 => SigHashType::None,
            0x03 => SigHashType::Single,
            _ => SigHashType::All,
        }
    }
}

// =============================================================================
// Script Types (Bitcoin-like output conditions)
// =============================================================================

/// The type of locking script on an output
/// Determines how the output can be spent
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScriptType {
    /// Pay to Public Key Hash (P2PKH) - most common
    /// Requires signature from the public key that hashes to the given address
    P2PKH,

    /// Pay to Script Hash (P2SH) - for complex scripts
    /// The redeem script hash is provided; actual script revealed when spending
    P2SH {
        /// SHA256 hash of the redeem script
        script_hash: String,
    },

    /// Pay to Witness Public Key Hash (P2WPKH) - SegWit native
    /// Like P2PKH but with witness data segregation
    P2WPKH,

    /// Pay to Witness Script Hash (P2WSH) - SegWit script
    /// Like P2SH but with witness data segregation
    P2WSH {
        /// SHA256 hash of the witness script
        script_hash: String,
    },

    /// Multi-signature (M-of-N)
    /// Requires M signatures from N possible signers
    MultiSig {
        /// Number of required signatures
        threshold: u8,
        /// Total number of possible signers
        total: u8,
        /// Public keys of all possible signers
        pubkeys: Vec<String>,
    },

    /// Check Lock Time Verify (CLTV) - time-locked
    /// Cannot be spent until block height or timestamp
    TimeLock {
        /// Lock time (block height if < 500M, timestamp if >= 500M)
        locktime: u32,
        /// Inner script type (what to check after timelock expires)
        inner: Box<ScriptType>,
    },

    /// Check Sequence Verify (CSV) - relative time-lock
    /// Cannot be spent until N blocks/seconds after confirmation
    RelativeTimeLock {
        /// Relative lock time in blocks or 512-second units
        sequence: u32,
        /// Inner script type
        inner: Box<ScriptType>,
    },

    /// OP_RETURN - Provably unspendable (data carrier)
    /// Used to embed data in the blockchain
    OpReturn {
        /// Embedded data (up to 80 bytes)
        data: Vec<u8>,
    },
}

impl Default for ScriptType {
    fn default() -> Self {
        ScriptType::P2PKH
    }
}

impl ScriptType {
    /// Create a new P2PKH script (default for most addresses)
    pub fn p2pkh() -> Self {
        ScriptType::P2PKH
    }

    /// Create a new multisig script
    pub fn multisig(threshold: u8, pubkeys: Vec<String>) -> Result<Self, ScriptError> {
        let total = pubkeys.len() as u8;
        if threshold == 0 || threshold > total || total > 20 {
            return Err(ScriptError::ThresholdNotMet(threshold, total, 0));
        }
        Ok(ScriptType::MultiSig {
            threshold,
            total,
            pubkeys,
        })
    }

    /// Create a time-locked script
    pub fn with_timelock(inner: ScriptType, locktime: u32) -> Self {
        ScriptType::TimeLock {
            locktime,
            inner: Box::new(inner),
        }
    }

    /// Create a relative time-locked script
    pub fn with_relative_timelock(inner: ScriptType, sequence: u32) -> Self {
        ScriptType::RelativeTimeLock {
            sequence,
            inner: Box::new(inner),
        }
    }

    /// Create an OP_RETURN output (data carrier, unspendable)
    pub fn op_return(data: Vec<u8>) -> Result<Self, ScriptError> {
        if data.len() > 80 {
            return Err(ScriptError::ScriptTooLarge(data.len()));
        }
        Ok(ScriptType::OpReturn { data })
    }

    /// Check if this script type is spendable
    pub fn is_spendable(&self) -> bool {
        !matches!(self, ScriptType::OpReturn { .. })
    }

    /// Check if this is a SegWit script type
    pub fn is_segwit(&self) -> bool {
        matches!(self, ScriptType::P2WPKH | ScriptType::P2WSH { .. })
    }

    /// Get the script type name
    pub fn type_name(&self) -> &'static str {
        match self {
            ScriptType::P2PKH => "P2PKH",
            ScriptType::P2SH { .. } => "P2SH",
            ScriptType::P2WPKH => "P2WPKH",
            ScriptType::P2WSH { .. } => "P2WSH",
            ScriptType::MultiSig { .. } => "MultiSig",
            ScriptType::TimeLock { .. } => "TimeLock",
            ScriptType::RelativeTimeLock { .. } => "RelativeTimeLock",
            ScriptType::OpReturn { .. } => "OP_RETURN",
        }
    }

    /// Estimate the size of this script in bytes
    pub fn estimated_size(&self) -> usize {
        match self {
            ScriptType::P2PKH => 25, // OP_DUP OP_HASH160 <20> OP_EQUALVERIFY OP_CHECKSIG
            ScriptType::P2SH { .. } => 23, // OP_HASH160 <20> OP_EQUAL
            ScriptType::P2WPKH => 22, // OP_0 <20>
            ScriptType::P2WSH { .. } => 34, // OP_0 <32>
            ScriptType::MultiSig { total, .. } => {
                // OP_M <pubkeys> OP_N OP_CHECKMULTISIG
                1 + (*total as usize * 34) + 1 + 1
            }
            ScriptType::TimeLock { inner, .. } => {
                // <locktime> OP_CHECKLOCKTIMEVERIFY OP_DROP <inner>
                5 + 1 + 1 + inner.estimated_size()
            }
            ScriptType::RelativeTimeLock { inner, .. } => {
                // <sequence> OP_CHECKSEQUENCEVERIFY OP_DROP <inner>
                5 + 1 + 1 + inner.estimated_size()
            }
            ScriptType::OpReturn { data } => 1 + 1 + data.len(), // OP_RETURN <len> <data>
        }
    }
}

// =============================================================================
// Script Validation
// =============================================================================

/// Validate if a script can be unlocked with given parameters
pub struct ScriptValidator {
    /// Current block height
    pub block_height: u64,
    /// Current block timestamp
    pub block_time: u64,
}

impl ScriptValidator {
    pub fn new(block_height: u64, block_time: u64) -> Self {
        Self {
            block_height,
            block_time,
        }
    }

    /// Check if a timelock has expired
    pub fn check_timelock(&self, locktime: u32) -> Result<(), ScriptError> {
        // Same threshold as Bitcoin (500 million)
        const LOCKTIME_THRESHOLD: u32 = 500_000_000;

        if locktime == 0 {
            return Ok(());
        }

        if locktime < LOCKTIME_THRESHOLD {
            // Block height lock
            if self.block_height < locktime as u64 {
                return Err(ScriptError::TimelockActive(locktime));
            }
        } else {
            // Timestamp lock
            if self.block_time < locktime as u64 {
                return Err(ScriptError::TimelockActive(locktime));
            }
        }

        Ok(())
    }

    /// Validate that a script type can be satisfied
    pub fn validate_script(&self, script: &ScriptType) -> Result<(), ScriptError> {
        match script {
            ScriptType::P2PKH | ScriptType::P2WPKH => Ok(()),
            ScriptType::P2SH { .. } | ScriptType::P2WSH { .. } => Ok(()),
            ScriptType::MultiSig {
                threshold, total, ..
            } => {
                if *threshold == 0 || *threshold > *total {
                    return Err(ScriptError::ThresholdNotMet(*threshold, *total, 0));
                }
                Ok(())
            }
            ScriptType::TimeLock { locktime, inner } => {
                self.check_timelock(*locktime)?;
                self.validate_script(inner)
            }
            ScriptType::RelativeTimeLock { inner, .. } => {
                // Relative timelocks are checked per-input, not here
                self.validate_script(inner)
            }
            ScriptType::OpReturn { .. } => Err(ScriptError::ExecutionFailed(
                "OP_RETURN is unspendable".to_string(),
            )),
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_script_type_default() {
        let script = ScriptType::default();
        assert!(matches!(script, ScriptType::P2PKH));
        assert!(script.is_spendable());
        assert!(!script.is_segwit());
    }

    #[test]
    fn test_sighash_types() {
        assert_eq!(SigHashType::default(), SigHashType::All);
        assert!(!SigHashType::All.is_anyone_can_pay());
        assert!(SigHashType::AllAnyoneCanPay.is_anyone_can_pay());
        assert_eq!(SigHashType::AllAnyoneCanPay.base_type(), SigHashType::All);
    }

    #[test]
    fn test_multisig_creation() {
        let pubkeys = vec!["pk1".to_string(), "pk2".to_string(), "pk3".to_string()];
        let script = ScriptType::multisig(2, pubkeys).unwrap();
        assert!(matches!(
            script,
            ScriptType::MultiSig {
                threshold: 2,
                total: 3,
                ..
            }
        ));
    }

    #[test]
    fn test_timelock() {
        let inner = ScriptType::P2PKH;
        let script = ScriptType::with_timelock(inner, 1000);
        assert!(matches!(
            script,
            ScriptType::TimeLock { locktime: 1000, .. }
        ));

        let validator = ScriptValidator::new(1001, 0);
        assert!(validator.validate_script(&script).is_ok());

        let validator = ScriptValidator::new(999, 0);
        assert!(validator.validate_script(&script).is_err());
    }

    #[test]
    fn test_op_return() {
        let data = b"Hello, blockchain!".to_vec();
        let script = ScriptType::op_return(data).unwrap();
        assert!(!script.is_spendable());
        assert_eq!(script.type_name(), "OP_RETURN");
    }

    #[test]
    fn test_script_size_estimation() {
        assert_eq!(ScriptType::P2PKH.estimated_size(), 25);
        assert_eq!(ScriptType::P2WPKH.estimated_size(), 22);
    }
}
