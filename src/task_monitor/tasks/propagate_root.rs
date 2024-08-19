use std::sync::Arc;

use crate::app::App;

pub async fn propagate_root(app: Arc<App>) -> anyhow::Result<()> {
    loop {
        app.bridge_processor
            .propagate_root()
            .await?;

        tokio::time::sleep(app.config.app.time_between_scans).await;
    }
}
