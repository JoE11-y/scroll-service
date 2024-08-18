//! Functionality for interacting with smart contracts deployed on chain.
pub mod abi;
pub mod scanner;

use anyhow::{anyhow, bail};
use ethers::providers::Middleware;
use ethers::types::U256;
use tracing::{error, info, instrument};

use self::abi::{ScrollStateBridge, ScrollWorldId, WorldId};
use crate::config::Config;
use crate::ethereum::{Ethereum, ReadProvider};
use crate::utils::TransactionId;

/// A structure representing the interface to the batch-based identity manager
/// contract.
#[derive(Debug)]
pub struct ScrollBridge {
    ethereum:       Ethereum,
    abi:            ScrollStateBridge<ReadProvider>,
    secondary_abi:  ScrollWorldId<ReadProvider>,
    tertiary_abi:   WorldId<ReadProvider>
}

impl ScrollBridge {
    // TODO: I don't like these public getters
    pub fn abi(&self) -> &ScrollStateBridge<ReadProvider> {
        &self.abi
    }

    pub fn secondary_abis(&self) -> &ScrollWorldId<ReadProvider> {
        &self.secondary_abi
    }

    pub fn tertiary_abis(&self) -> &WorldId<ReadProvider> {
      &self.tertiary_abi
  }

    #[instrument(level = "debug", skip_all)]
    pub async fn new(config: &Config, ethereum: Ethereum) -> anyhow::Result<Self>
    where
        Self: Sized,
    {
        let Some(network_config) = &config.network else {
            bail!("Network config is required for ScrollBridge.");
        };

        // Check that there is code deployed at the target address.
        let address = network_config.scroll_bridge_address;
        let code = ethereum.provider().get_code(address, None).await?;
        if code.as_ref().is_empty() {
            error!(
                ?address,
                "No contract code is deployed at the provided address."
            );
        }

        // Connect to the running batching contract.
        let abi = ScrollStateBridge::new(
            network_config.scroll_bridge_address,
            ethereum.provider().clone(),
        );

        let owner = abi.owner().call().await?;
        if owner != ethereum.address() {
            error!(?owner, signer = ?ethereum.address(), "Signer is not the owner of the state bridge contract.");
            panic!("Cannot currently continue in read-only mode.")
        }

        info!(
            ?address,
            ?owner,
            "Connected to the Scroll State WorldID Bridge"
        );

        // get scrollworldID address from scoll bridge
        let scrollWorldIdAddress = abi.scroll_world_id_address().call().await?;

        let code = ethereum.provider().get_code(scrollWorldIdAddress, None).await?;
        if code.as_ref().is_empty() {
            error!(
                ?scrollWorldIdAddress,
                "No contract code is deployed at the scroll world id address."
            );
        }
        let secondary_abi = ScrollWorldId::new(
            scrollWorldIdAddress,
            ethereum.provider().clone()
        );

        // get worldId address from scroll bridge
        let worldIdAddess = abi.world_id_address().call().await?;
        let world_id_provider = ethereum.secondary_provider();
        let tertiary_abi = WorldId::new(
            worldIdAddess,
            world_id_provider.clone()
        );

        let scroll_bridge = Self {
            ethereum,
            abi,
            secondary_abi,
            tertiary_abi
        };

        Ok(scroll_bridge)
    }

    #[instrument(level = "debug")]
    pub async fn propagate_root(&self) -> anyhow::Result<TransactionId> {
        let propagate_root_transaction = self.abi.propagate_root().tx;
        self.ethereum
            .send_transaction(propagate_root_transaction, true)
            .await
            .map_err(|tx_err| anyhow!("{}", tx_err.to_string()))
    }

    #[instrument(level = "debug", skip_all)]
    pub async fn get_scroll_latest_root(&self) -> anyhow::Result<U256> {
        let latest_root = self.secondary_abi.latest_root().call().await?;
        Ok(latest_root)
    }

    #[instrument(level = "debug", skip_all)]
    pub async fn get_world_id_latest_root(&self) -> anyhow::Result<U256> {
        let latest_root = self.tertiary_abi.latest_root().call().await?;
        Ok(latest_root)
    }

    #[instrument(level = "debug", skip_all)]
    pub async fn is_root_mined(&self, root: U256) -> anyhow::Result<bool> {
        let (root_on_mainnet, ..) = self.tertiary_abi.query_root(root).call().await?;

        if root_on_mainnet.is_zero() {
            return Ok(false);
        }

        let root_timestamp = self.secondary_abi.root_history(root).call().await?;

        // root_history only returns superseded roots, so we must also check the latest
        // root
        let latest_root = self.secondary_abi.latest_root().call().await?;

        // If root is not superseded and it's not the latest root
        // then it's not mined
        if root_timestamp == 0 && root != latest_root {
            return Ok(false);
        }

        Ok(true)
    }
}
