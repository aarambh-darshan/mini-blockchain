//! Smart contract opcodes
//!
//! Defines the instruction set for the contract virtual machine.

use serde::{Deserialize, Serialize};

/// Opcodes for the smart contract VM
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum OpCode {
    // Stack operations (0x00 - 0x0F)
    /// Push a value onto the stack
    Push = 0x00,
    /// Pop the top value from the stack
    Pop = 0x01,
    /// Duplicate the top value
    Dup = 0x02,
    /// Swap the top two values
    Swap = 0x03,

    // Arithmetic (0x10 - 0x1F)
    /// Add top two values
    Add = 0x10,
    /// Subtract top from second
    Sub = 0x11,
    /// Multiply top two values
    Mul = 0x12,
    /// Divide second by top
    Div = 0x13,
    /// Modulo second by top
    Mod = 0x14,

    // Comparison (0x20 - 0x2F)
    /// Equal: push 1 if equal, 0 otherwise
    Eq = 0x20,
    /// Less than
    Lt = 0x21,
    /// Greater than
    Gt = 0x22,
    /// Less than or equal
    Le = 0x23,
    /// Greater than or equal
    Ge = 0x24,
    /// Not equal
    Neq = 0x25,
    /// Check if zero
    IsZero = 0x26,

    // Logic (0x30 - 0x3F)
    /// Logical AND
    And = 0x30,
    /// Logical OR
    Or = 0x31,
    /// Logical NOT
    Not = 0x32,

    // Control flow (0x40 - 0x4F)
    /// Unconditional jump
    Jump = 0x40,
    /// Conditional jump (if top of stack is non-zero)
    JumpIf = 0x41,
    /// Halt execution
    Halt = 0x42,
    /// Return with value
    Return = 0x43,
    /// Revert execution
    Revert = 0x44,

    // Storage (0x50 - 0x5F)
    /// Store value: key, value -> storage
    SStore = 0x50,
    /// Load value: key -> value
    SLoad = 0x51,

    // Blockchain context (0x60 - 0x6F)
    /// Get balance of address
    Balance = 0x60,
    /// Transfer coins: to, amount -> success
    Transfer = 0x61,
    /// Push caller address
    Caller = 0x62,
    /// Push contract address
    Self_ = 0x63,
    /// Push current block timestamp
    Timestamp = 0x64,
    /// Push current block number
    BlockNumber = 0x65,
    /// Push contract's balance
    SelfBalance = 0x66,

    // Arguments (0x70 - 0x7F)
    /// Load argument by index
    Arg = 0x70,
    /// Get number of arguments
    ArgCount = 0x71,

    // No operation
    Nop = 0xFF,
}

impl OpCode {
    /// Convert byte to opcode
    pub fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            0x00 => Some(OpCode::Push),
            0x01 => Some(OpCode::Pop),
            0x02 => Some(OpCode::Dup),
            0x03 => Some(OpCode::Swap),
            0x10 => Some(OpCode::Add),
            0x11 => Some(OpCode::Sub),
            0x12 => Some(OpCode::Mul),
            0x13 => Some(OpCode::Div),
            0x14 => Some(OpCode::Mod),
            0x20 => Some(OpCode::Eq),
            0x21 => Some(OpCode::Lt),
            0x22 => Some(OpCode::Gt),
            0x23 => Some(OpCode::Le),
            0x24 => Some(OpCode::Ge),
            0x25 => Some(OpCode::Neq),
            0x26 => Some(OpCode::IsZero),
            0x30 => Some(OpCode::And),
            0x31 => Some(OpCode::Or),
            0x32 => Some(OpCode::Not),
            0x40 => Some(OpCode::Jump),
            0x41 => Some(OpCode::JumpIf),
            0x42 => Some(OpCode::Halt),
            0x43 => Some(OpCode::Return),
            0x44 => Some(OpCode::Revert),
            0x50 => Some(OpCode::SStore),
            0x51 => Some(OpCode::SLoad),
            0x60 => Some(OpCode::Balance),
            0x61 => Some(OpCode::Transfer),
            0x62 => Some(OpCode::Caller),
            0x63 => Some(OpCode::Self_),
            0x64 => Some(OpCode::Timestamp),
            0x65 => Some(OpCode::BlockNumber),
            0x66 => Some(OpCode::SelfBalance),
            0x70 => Some(OpCode::Arg),
            0x71 => Some(OpCode::ArgCount),
            0xFF => Some(OpCode::Nop),
            _ => None,
        }
    }

    /// Get the number of bytes this opcode consumes after itself
    pub fn arg_bytes(&self) -> usize {
        match self {
            OpCode::Push => 8,   // 64-bit value
            OpCode::Jump => 4,   // 32-bit offset
            OpCode::JumpIf => 4, // 32-bit offset
            OpCode::Arg => 1,    // 8-bit index
            _ => 0,
        }
    }

    /// Get opcode name for disassembly
    pub fn name(&self) -> &'static str {
        match self {
            OpCode::Push => "PUSH",
            OpCode::Pop => "POP",
            OpCode::Dup => "DUP",
            OpCode::Swap => "SWAP",
            OpCode::Add => "ADD",
            OpCode::Sub => "SUB",
            OpCode::Mul => "MUL",
            OpCode::Div => "DIV",
            OpCode::Mod => "MOD",
            OpCode::Eq => "EQ",
            OpCode::Lt => "LT",
            OpCode::Gt => "GT",
            OpCode::Le => "LE",
            OpCode::Ge => "GE",
            OpCode::Neq => "NEQ",
            OpCode::IsZero => "ISZERO",
            OpCode::And => "AND",
            OpCode::Or => "OR",
            OpCode::Not => "NOT",
            OpCode::Jump => "JUMP",
            OpCode::JumpIf => "JUMPI",
            OpCode::Halt => "HALT",
            OpCode::Return => "RETURN",
            OpCode::Revert => "REVERT",
            OpCode::SStore => "SSTORE",
            OpCode::SLoad => "SLOAD",
            OpCode::Balance => "BALANCE",
            OpCode::Transfer => "TRANSFER",
            OpCode::Caller => "CALLER",
            OpCode::Self_ => "SELF",
            OpCode::Timestamp => "TIMESTAMP",
            OpCode::BlockNumber => "BLOCKNUMBER",
            OpCode::SelfBalance => "SELFBALANCE",
            OpCode::Arg => "ARG",
            OpCode::ArgCount => "ARGCOUNT",
            OpCode::Nop => "NOP",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_opcode_roundtrip() {
        let opcodes = [
            OpCode::Push,
            OpCode::Add,
            OpCode::Eq,
            OpCode::Jump,
            OpCode::SStore,
            OpCode::Caller,
        ];

        for op in opcodes {
            let byte = op as u8;
            let decoded = OpCode::from_byte(byte).unwrap();
            assert_eq!(op, decoded);
        }
    }
}
