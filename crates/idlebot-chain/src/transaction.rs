use anyhow::Result;

/// Types for contract interactions

/// Builder for contract call data
#[derive(Debug, Clone)]
pub struct ContractCall {
    pub contract_address: String,
    pub method: String,
    pub params: Vec<String>,
}

impl ContractCall {
    /// Create a new contract call
    pub fn new(contract_address: &str, method: &str) -> Self {
        Self {
            contract_address: contract_address.to_string(),
            method: method.to_string(),
            params: Vec::new(),
        }
    }

    /// Add a parameter
    pub fn param(mut self, param: &str) -> Self {
        self.params.push(param.to_string());
        self
    }

    /// Encode the call data (placeholder — will use alloy-contract ABI encoding)
    pub fn encode(&self) -> Result<Vec<u8>> {
        // TODO: implement ABI encoding in phase 2
        Ok(Vec::new())
    }
}

/// Struct for representing a blockchain transaction
#[derive(Debug, Clone)]
pub struct Transaction {
    pub tx_hash: String,
    pub from: String,
    pub to: String,
    pub value: u64,
    pub data: Vec<u8>,
    pub timestamp: u64,
    pub status: TxStatus,
}

/// Transaction lifecycle status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TxStatus {
    Pending,
    Confirmed,
    Failed,
}

/// A contract event emitted on-chain
#[derive(Debug, Clone)]
pub struct ContractEvent {
    pub contract_address: String,
    pub event_name: String,
    pub params: Vec<String>,
    pub block_number: u64,
    pub tx_hash: String,
}
