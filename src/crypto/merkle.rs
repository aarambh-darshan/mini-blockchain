//! Merkle tree implementation for transaction verification
//!
//! Provides efficient verification of transaction inclusion in blocks
//! using a binary hash tree structure.

use super::hash::sha256;

/// Calculate the merkle root from a list of transaction hashes
pub fn calculate_merkle_root(hashes: &[Vec<u8>]) -> Vec<u8> {
    if hashes.is_empty() {
        return sha256(b"");
    }

    if hashes.len() == 1 {
        return hashes[0].clone();
    }

    let mut current_level: Vec<Vec<u8>> = hashes.to_vec();

    while current_level.len() > 1 {
        let mut next_level = Vec::new();

        // Process pairs of hashes
        for chunk in current_level.chunks(2) {
            let combined = if chunk.len() == 2 {
                // Concatenate two hashes
                let mut data = chunk[0].clone();
                data.extend_from_slice(&chunk[1]);
                sha256(&data)
            } else {
                // Duplicate the last hash if odd number
                let mut data = chunk[0].clone();
                data.extend_from_slice(&chunk[0]);
                sha256(&data)
            };
            next_level.push(combined);
        }

        current_level = next_level;
    }

    current_level.remove(0)
}

/// Calculate merkle root from hex-encoded hashes
pub fn calculate_merkle_root_hex(hex_hashes: &[String]) -> String {
    let hashes: Vec<Vec<u8>> = hex_hashes
        .iter()
        .filter_map(|h| hex::decode(h).ok())
        .collect();
    hex::encode(calculate_merkle_root(&hashes))
}

/// A node in the merkle tree
#[derive(Debug, Clone)]
pub struct MerkleNode {
    pub hash: Vec<u8>,
    pub left: Option<Box<MerkleNode>>,
    pub right: Option<Box<MerkleNode>>,
}

impl MerkleNode {
    /// Create a leaf node
    pub fn leaf(hash: Vec<u8>) -> Self {
        Self {
            hash,
            left: None,
            right: None,
        }
    }

    /// Create an internal node from two children
    pub fn internal(left: MerkleNode, right: MerkleNode) -> Self {
        let mut combined = left.hash.clone();
        combined.extend_from_slice(&right.hash);
        let hash = sha256(&combined);

        Self {
            hash,
            left: Some(Box::new(left)),
            right: Some(Box::new(right)),
        }
    }
}

/// Build a complete merkle tree and return the root node
pub fn build_merkle_tree(hashes: &[Vec<u8>]) -> Option<MerkleNode> {
    if hashes.is_empty() {
        return None;
    }

    // Create leaf nodes
    let mut nodes: Vec<MerkleNode> = hashes.iter().map(|h| MerkleNode::leaf(h.clone())).collect();

    // Build tree bottom-up
    while nodes.len() > 1 {
        let mut next_level = Vec::new();

        for chunk in nodes.chunks(2) {
            let node = if chunk.len() == 2 {
                MerkleNode::internal(chunk[0].clone(), chunk[1].clone())
            } else {
                MerkleNode::internal(chunk[0].clone(), chunk[0].clone())
            };
            next_level.push(node);
        }

        nodes = next_level;
    }

    nodes.into_iter().next()
}

/// Merkle proof for verifying transaction inclusion
#[derive(Debug, Clone)]
pub struct MerkleProof {
    /// List of sibling hashes from leaf to root
    pub siblings: Vec<(Vec<u8>, bool)>, // (hash, is_left)
}

impl MerkleProof {
    /// Verify the proof against a root hash
    pub fn verify(&self, leaf_hash: &[u8], root_hash: &[u8]) -> bool {
        let mut current = leaf_hash.to_vec();

        for (sibling, is_left) in &self.siblings {
            let combined = if *is_left {
                let mut data = sibling.clone();
                data.extend_from_slice(&current);
                data
            } else {
                let mut data = current.clone();
                data.extend_from_slice(sibling);
                data
            };
            current = sha256(&combined);
        }

        current == root_hash
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merkle_root_single() {
        let hashes = vec![sha256(b"tx1")];
        let root = calculate_merkle_root(&hashes);
        assert_eq!(root, hashes[0]);
    }

    #[test]
    fn test_merkle_root_two() {
        let hash1 = sha256(b"tx1");
        let hash2 = sha256(b"tx2");
        let hashes = vec![hash1.clone(), hash2.clone()];

        let root = calculate_merkle_root(&hashes);

        let mut expected = hash1;
        expected.extend_from_slice(&hash2);
        let expected_root = sha256(&expected);

        assert_eq!(root, expected_root);
    }

    #[test]
    fn test_merkle_root_odd() {
        let hashes = vec![sha256(b"tx1"), sha256(b"tx2"), sha256(b"tx3")];
        let root = calculate_merkle_root(&hashes);
        assert_eq!(root.len(), 32);
    }

    #[test]
    fn test_build_merkle_tree() {
        let hashes = vec![sha256(b"tx1"), sha256(b"tx2"), sha256(b"tx3"), sha256(b"tx4")];
        let tree = build_merkle_tree(&hashes);
        assert!(tree.is_some());
        
        let root = tree.unwrap();
        assert_eq!(root.hash, calculate_merkle_root(&hashes));
    }

    #[test]
    fn test_empty_merkle_root() {
        let hashes: Vec<Vec<u8>> = vec![];
        let root = calculate_merkle_root(&hashes);
        assert_eq!(root, sha256(b""));
    }
}
