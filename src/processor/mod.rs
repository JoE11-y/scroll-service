use std::sync::Arc;

use async_trait::async_trait;
use ethers::abi::RawLog;
use ethers::addressbook::Address;
use ethers::contract::EthEvent;
use ethers::middleware::Middleware;
use ethers::prelude::{Log, Topic, ValueOrArray, U256};
use tracing::{error, info, instrument};

pub mod status;

use crate::config::Config;
use crate::contracts::abi::{RootAddedFilter, RootPropagatedFilter};
use crate::contracts::scanner::BlockScanner;
use crate::contracts::ScrollBridge;
use crate::database::Database;
use crate::ethereum::{Ethereum, ReadProvider};

pub type TransactionId = String;

#[async_trait]
pub trait Processor: Send + Sync + 'static {
    async fn propagate_root(&self) -> anyhow::Result<TransactionId>;
    async fn await_clean_slate(&self) -> anyhow::Result<()>;
    async fn mine_transaction(&self, transaction_id: TransactionId) -> anyhow::Result<bool>;
}

pub struct BridgeProcessor {
    ethereum:           Ethereum,
    config:             Config,
    database:           Arc<Database>,
    scroll_bridge:      Arc<ScrollBridge>,

    bridge_scanner:     tokio::sync::Mutex<BlockScanner<Arc<ReadProvider>>>,
    bridge_address:     Address,
    scroll_world_id_scanner:   tokio::sync::Mutex<BlockScanner<Arc<ReadProvider>>>,
    scroll_world_id_address: Address,
}

#[async_trait]
impl Processor for BridgeProcessor {
    async fn propagate_root(&self) -> anyhow::Result<TransactionId> {
        self.propagate_root().await
    }

    async fn await_clean_slate(&self) -> anyhow::Result<()> {
        // Await for all pending transactions
        let pending_identities = self.fetch_pending_identities().await?;

        for pending_identity_tx in pending_identities {
            // Ignores the result of each transaction - we only care about a clean slate in
            // terms of pending transactions
            drop(self.mine_transaction(pending_identity_tx).await);
        }
        Ok(())
    }

    #[instrument(level = "debug", skip(self))]
    async fn mine_transaction(&self, transaction_id: TransactionId) -> anyhow::Result<bool> {
        let result = self.ethereum.mine_transaction(transaction_id).await?;

        Ok(result)
    }
}

impl BridgeProcessor {
    pub async fn new(
        ethereum: Ethereum,
        database: Arc<Database>,
        config: Config,
        scroll_bridge: Arc<ScrollBridge>
    ) -> anyhow::Result<Self> {
        let bridge_abi = scroll_bridge.bridge_abi();
        let scroll_world_id_abi = scroll_bridge.scroll_world_id_abi();
        // let world_id_abi: &WorldId<ReadProvider> = scroll_bridge.world_id_abi();

        let bridge_scanner = tokio::sync::Mutex::new(
            BlockScanner::new_latest(
                bridge_abi.client().clone(),
                config.app.scanning_window_size,
            )
            .await?
            .with_offset(config.app.scanning_chain_head_offset),
        );

        let scroll_world_id_scanner = tokio::sync::Mutex::new(
          BlockScanner::new_latest(
              scroll_world_id_abi.client().clone(),
              config.app.scanning_window_size,
          )
          .await?
          .with_offset(config.app.scanning_chain_head_offset),
      );

        let bridge_address = bridge_abi.address();
        let scroll_world_id_address = scroll_world_id_abi.address();
        Ok(Self {
            ethereum,
            config,
            database,
            scroll_bridge,
            bridge_scanner,
            bridge_address,
            scroll_world_id_scanner,
            scroll_world_id_address
        })
    }


    #[instrument(level = "info", skip_all)]
    async fn propagate_root(
        &self,
    ) -> anyhow::Result<TransactionId> {

        info!("Creating propagation root");

        let transaction_id = self
            .scroll_bridge
            .propagate_root()
            .await
            .map_err(|e| {
                error!(?e, "Failed to propagate root");
                e
            })?;

        info!(
            ?transaction_id,
            "Progation submitted"
        );

        Ok(transaction_id)
    }


    #[instrument(level = "debug", skip_all)]
    async fn fetch_pending_identities(&self) -> anyhow::Result<Vec<TransactionId>> {
        let pending_identities = self.ethereum.fetch_pending_transactions().await?;

        Ok(pending_identities)
    }

    async fn fetch_bridge_logs(&self) -> anyhow::Result<Vec<Log>>
    where
        <ReadProvider as Middleware>::Error: 'static,
    {
        let bridge_topics = [
            Some(Topic::from(RootPropagatedFilter::signature())),
            None,
            None,
            None,
        ];

        let bridge_address = Some(ValueOrArray::Value(self.bridge_address));

        let mut bridge_scanner = self.bridge_scanner.lock().await;

        let bridge_logs = bridge_scanner
            .next(bridge_address, bridge_topics.clone())
            .await?;

        Ok(bridge_logs)
    }

    async fn fetch_scroll_logs(&self) -> anyhow::Result<Vec<Log>>
    where
        <ReadProvider as Middleware>::Error: 'static,
    {
        let bridged_topics = [
            Some(Topic::from(RootAddedFilter::signature())),
            None,
            None,
            None,
        ];
        
        let mut scroll_scanner = self.scroll_world_id_scanner.lock().await;

        let logs = scroll_scanner
            .next(Some(ValueOrArray::Value(self.scroll_world_id_address)), bridged_topics.clone())
            .await?;

        Ok(logs)
    }

    fn extract_roots_from_scroll_logs(logs: &[Log]) -> Vec<U256> {
        let mut roots = vec![];

        for log in logs {
            let raw_log = RawLog::from((log.topics.clone(), log.data.to_vec()));
            if let Ok(event) = RootAddedFilter::decode_log(&raw_log) {
                roots.push(event.root);
            }
        }

        roots
    }

    fn extract_roots_from_bridge_logs(logs: &[Log]) -> Vec<U256> {
        let mut roots = vec![];

        for log in logs {
            let raw_log = RawLog::from((log.topics.clone(), log.data.to_vec()));
            if let Ok(event) = RootPropagatedFilter::decode_log(&raw_log) {
                roots.push(event.root);
            }
        }

        roots
    }
}

