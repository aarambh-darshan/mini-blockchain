//! Simple contract compiler
//!
//! Compiles assembly-like syntax to bytecode.

use crate::contract::opcodes::OpCode;
use std::collections::HashMap;
use thiserror::Error;

/// Compiler errors
#[derive(Error, Debug)]
pub enum CompilerError {
    #[error("Unknown instruction: {0}")]
    UnknownInstruction(String),
    #[error("Invalid argument: {0}")]
    InvalidArgument(String),
    #[error("Undefined label: {0}")]
    UndefinedLabel(String),
    #[error("Invalid number: {0}")]
    InvalidNumber(String),
}

/// Simple compiler for contract bytecode
pub struct Compiler {
    /// Output bytecode
    code: Vec<u8>,
    /// Label positions
    labels: HashMap<String, u32>,
    /// Pending label references (position, label_name)
    label_refs: Vec<(usize, String)>,
}

impl Compiler {
    /// Create a new compiler
    pub fn new() -> Self {
        Self {
            code: Vec::new(),
            labels: HashMap::new(),
            label_refs: Vec::new(),
        }
    }

    /// Compile source code to bytecode
    pub fn compile(&mut self, source: &str) -> Result<Vec<u8>, CompilerError> {
        self.code.clear();
        self.labels.clear();
        self.label_refs.clear();

        // First pass: collect labels
        for line in source.lines() {
            let line = line.trim();

            // Skip empty lines and comments
            if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
                continue;
            }

            // Check for label definition
            if line.starts_with(':') {
                let label = line[1..].trim().to_string();
                self.labels.insert(label, self.code.len() as u32);
                continue;
            }

            // Parse instruction
            self.compile_instruction(line)?;
        }

        // Second pass: resolve label references
        for (pos, label) in &self.label_refs {
            let addr = self
                .labels
                .get(label)
                .ok_or_else(|| CompilerError::UndefinedLabel(label.clone()))?;
            let bytes = addr.to_be_bytes();
            self.code[*pos] = bytes[0];
            self.code[*pos + 1] = bytes[1];
            self.code[*pos + 2] = bytes[2];
            self.code[*pos + 3] = bytes[3];
        }

        Ok(self.code.clone())
    }

    /// Compile a single instruction
    fn compile_instruction(&mut self, line: &str) -> Result<(), CompilerError> {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.is_empty() {
            return Ok(());
        }

        let instruction = parts[0].to_uppercase();

        match instruction.as_str() {
            // Stack operations
            "PUSH" => {
                self.code.push(OpCode::Push as u8);
                let value = self.parse_number(parts.get(1).unwrap_or(&"0"))?;
                self.code.extend_from_slice(&value.to_be_bytes());
            }
            "POP" => self.code.push(OpCode::Pop as u8),
            "DUP" => self.code.push(OpCode::Dup as u8),
            "SWAP" => self.code.push(OpCode::Swap as u8),

            // Arithmetic
            "ADD" => self.code.push(OpCode::Add as u8),
            "SUB" => self.code.push(OpCode::Sub as u8),
            "MUL" => self.code.push(OpCode::Mul as u8),
            "DIV" => self.code.push(OpCode::Div as u8),
            "MOD" => self.code.push(OpCode::Mod as u8),

            // Comparison
            "EQ" => self.code.push(OpCode::Eq as u8),
            "LT" => self.code.push(OpCode::Lt as u8),
            "GT" => self.code.push(OpCode::Gt as u8),
            "LE" => self.code.push(OpCode::Le as u8),
            "GE" => self.code.push(OpCode::Ge as u8),
            "NEQ" => self.code.push(OpCode::Neq as u8),
            "ISZERO" => self.code.push(OpCode::IsZero as u8),

            // Logic
            "AND" => self.code.push(OpCode::And as u8),
            "OR" => self.code.push(OpCode::Or as u8),
            "NOT" => self.code.push(OpCode::Not as u8),

            // Control flow
            "JUMP" => {
                self.code.push(OpCode::Jump as u8);
                let label = parts.get(1).ok_or_else(|| {
                    CompilerError::InvalidArgument("JUMP requires label".to_string())
                })?;
                self.label_refs.push((self.code.len(), label.to_string()));
                self.code.extend_from_slice(&[0, 0, 0, 0]); // Placeholder
            }
            "JUMPI" => {
                self.code.push(OpCode::JumpIf as u8);
                let label = parts.get(1).ok_or_else(|| {
                    CompilerError::InvalidArgument("JUMPI requires label".to_string())
                })?;
                self.label_refs.push((self.code.len(), label.to_string()));
                self.code.extend_from_slice(&[0, 0, 0, 0]); // Placeholder
            }
            "HALT" => self.code.push(OpCode::Halt as u8),
            "RETURN" => self.code.push(OpCode::Return as u8),
            "REVERT" => self.code.push(OpCode::Revert as u8),

            // Storage
            "SSTORE" => self.code.push(OpCode::SStore as u8),
            "SLOAD" => self.code.push(OpCode::SLoad as u8),

            // Blockchain
            "BALANCE" => self.code.push(OpCode::Balance as u8),
            "TRANSFER" => self.code.push(OpCode::Transfer as u8),
            "CALLER" => self.code.push(OpCode::Caller as u8),
            "SELF" => self.code.push(OpCode::Self_ as u8),
            "TIMESTAMP" => self.code.push(OpCode::Timestamp as u8),
            "BLOCKNUMBER" => self.code.push(OpCode::BlockNumber as u8),
            "SELFBALANCE" => self.code.push(OpCode::SelfBalance as u8),

            // Arguments
            "ARG" => {
                self.code.push(OpCode::Arg as u8);
                let index = parts
                    .get(1)
                    .ok_or_else(|| {
                        CompilerError::InvalidArgument("ARG requires index".to_string())
                    })?
                    .parse::<u8>()
                    .map_err(|_| CompilerError::InvalidNumber(parts[1].to_string()))?;
                self.code.push(index);
            }
            "ARGCOUNT" => self.code.push(OpCode::ArgCount as u8),

            "NOP" => self.code.push(OpCode::Nop as u8),

            _ => return Err(CompilerError::UnknownInstruction(instruction)),
        }

        Ok(())
    }

    /// Parse a number (decimal or hex)
    fn parse_number(&self, s: &str) -> Result<u64, CompilerError> {
        let s = s.trim();
        if s.starts_with("0x") || s.starts_with("0X") {
            u64::from_str_radix(&s[2..], 16)
                .map_err(|_| CompilerError::InvalidNumber(s.to_string()))
        } else {
            s.parse::<u64>()
                .map_err(|_| CompilerError::InvalidNumber(s.to_string()))
        }
    }
}

impl Default for Compiler {
    fn default() -> Self {
        Self::new()
    }
}

/// Disassemble bytecode to readable format
pub fn disassemble(code: &[u8]) -> String {
    let mut output = String::new();
    let mut pc = 0;

    while pc < code.len() {
        let opcode_byte = code[pc];
        if let Some(opcode) = OpCode::from_byte(opcode_byte) {
            output.push_str(&format!("{:04x}: {}", pc, opcode.name()));

            pc += 1;

            match opcode {
                OpCode::Push => {
                    if pc + 8 <= code.len() {
                        let bytes = [
                            code[pc],
                            code[pc + 1],
                            code[pc + 2],
                            code[pc + 3],
                            code[pc + 4],
                            code[pc + 5],
                            code[pc + 6],
                            code[pc + 7],
                        ];
                        let value = u64::from_be_bytes(bytes);
                        output.push_str(&format!(" {}", value));
                        pc += 8;
                    }
                }
                OpCode::Jump | OpCode::JumpIf => {
                    if pc + 4 <= code.len() {
                        let bytes = [code[pc], code[pc + 1], code[pc + 2], code[pc + 3]];
                        let addr = u32::from_be_bytes(bytes);
                        output.push_str(&format!(" 0x{:04x}", addr));
                        pc += 4;
                    }
                }
                OpCode::Arg => {
                    if pc < code.len() {
                        output.push_str(&format!(" {}", code[pc]));
                        pc += 1;
                    }
                }
                _ => {}
            }

            output.push('\n');
        } else {
            output.push_str(&format!("{:04x}: UNKNOWN 0x{:02x}\n", pc, opcode_byte));
            pc += 1;
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compile_simple() {
        let mut compiler = Compiler::new();
        let code = compiler
            .compile(
                "
            PUSH 42
            RETURN
        ",
            )
            .unwrap();

        assert!(!code.is_empty());
        assert_eq!(code[0], OpCode::Push as u8);
    }

    #[test]
    fn test_compile_with_labels() {
        let mut compiler = Compiler::new();
        let code = compiler
            .compile(
                "
            PUSH 1
            JUMPI end
            PUSH 999
            :end
            PUSH 42
            RETURN
        ",
            )
            .unwrap();

        assert!(!code.is_empty());
    }

    #[test]
    fn test_compile_arithmetic() {
        let mut compiler = Compiler::new();
        let code = compiler
            .compile(
                "
            PUSH 10
            PUSH 20
            ADD
            RETURN
        ",
            )
            .unwrap();

        // Verify the assembled code
        assert_eq!(code[0], OpCode::Push as u8);
        assert_eq!(code[9], OpCode::Push as u8);
        assert_eq!(code[18], OpCode::Add as u8);
        assert_eq!(code[19], OpCode::Return as u8);
    }

    #[test]
    fn test_disassemble() {
        let mut compiler = Compiler::new();
        let code = compiler
            .compile(
                "
            PUSH 42
            RETURN
        ",
            )
            .unwrap();

        let disasm = disassemble(&code);
        assert!(disasm.contains("PUSH"));
        assert!(disasm.contains("42"));
        assert!(disasm.contains("RETURN"));
    }
}
