use std::sync::Arc;

use tracing::instrument;

use crate::config::Config;
use crate::contracts::ScrollBridge;
use crate::processor::{Processor, BridgeProcessor};
use crate::ethereum::Ethereum;

pub struct App {
    pub config: Config,
    pub bridge_processor: Arc<dyn Processor>,
}

use crate::server::error::Error as ServerError;

impl App {
    /// # Errors
    ///
    /// Will return `Err` if the internal Ethereum handler errors
    ///
    #[instrument(name = "App::new", level = "debug", skip_all)]
    pub async fn new(config: Config) -> anyhow::Result<Arc<Self>> {
        let ethereum = Ethereum::new(&config).await?;
        let scroll_bridge = Arc::new(ScrollBridge::new(&config, ethereum.clone()).await?);
        let bridge_processor = Arc::new(
            BridgeProcessor::new(
                ethereum.clone(),
                config.clone(),
                scroll_bridge.clone()   
            )
            .await?
        );
        let app = Arc::new(Self {
            config,
            bridge_processor
        });
        Ok(app)
    }
}
