pub mod auth;
pub mod transaction;

use alloy::primitives::Address;
use alloy::providers::{ProviderBuilder, RootProvider};
use alloy::rpc::types::TransactionRequest;
use alloy::signers::local::LocalSigner;
use alloy::transport_http::Http;
use anyhow::{Context, Result};
use std::sync::Arc;
use transaction::ContractCall;

/// Blockchain configuration
pub struct ChainConfig {
    pub rpc_url: String,
    pub chain_id: u64,
    pub wallet_address: String,
    pub private_key: String,
}

impl ChainConfig {
    pub fn new(rpc_url: &str, chain_id: u64, wallet_address: &str, private_key: &str) -> Self {
        Self {
            rpc_url: rpc_url.to_string(),
            chain_id,
            wallet_address: wallet_address.to_string(),
            private_key: private_key.to_string(),
        }
    }

    /// Default Polygon testnet config
    pub fn polygon_amoy() -> Self {
        Self::new(
            "https://rpc.ankr.com/polygon_amoy",
            80002, // Amoy testnet
            "",
            "",
        )
    }
}

/// Blockchain provider for Polygon
pub struct ChainProvider {
    config: ChainConfig,
    signer: LocalSigner,
    provider: Arc<RootProvider<Http>>,
}

impl ChainProvider {
    pub fn new(config: ChainConfig) -> Result<Self> {
        let signer = LocalSigner::from_hex_pk(&config.private_key)
            .context("Failed to create signer from private key")?;

        let provider = ProviderBuilder::new()
            .with_recommended_fillers()
            .wallet(signer.clone())
            .on_http(config.rpc_url.parse().context("Invalid RPC URL")?);

        Ok(Self {
            config,
            signer,
            provider,
        })
    }

    /// Send a transaction
    pub async fn send_transaction(
        &self,
        to: Address,
        value: u64,
        data: Vec<u8>,
    ) -> Result<String> {
        let tx = TransactionRequest::default()
            .to(to)
            .value(alloy::primitives::U256::from(value))
            .input(data.into());

        let receipt = self.provider.send_transaction(tx).await?;
        Ok(hex::encode(receipt.transaction_hash))
    }

    /// Call a contract function (read-only)
    pub async fn call_contract(&self, call: ContractCall) -> Result<String> {
        // TODO: implement with alloy-contract in phase 2
        Ok(String::new())
    }

    /// Get account balance
    pub async fn get_balance(&self, address: Address) -> Result<u64> {
        let balance = self.provider.get_balance(address).await?;
        Ok(balance.as_limbs()[0])
    }

    /// Get transaction count (nonce)
    pub async fn get_nonce(&self) -> Result<u64> {
        let nonce = self
            .provider
            .get_transaction_count(self.signer.address())
            .await?;
        Ok(nonce.as_u64())
    }

    /// Get wallet address
    pub fn wallet_address(&self) -> &str {
        &self.config.wallet_address
    }
}
