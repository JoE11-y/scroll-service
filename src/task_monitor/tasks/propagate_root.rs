use std::sync::Arc;
use tokio::sync::{mpsc, Notify};
use crate::task_monitor::{App, TaskMonitor};
use crate::utils::TransactionId;

pub async fn propagate_root(
    app: Arc<App>, 
    monitored_txs_sender: Arc<mpsc::Sender<TransactionId>>,
    wake_up_notify: Arc<Notify>
) -> anyhow::Result<()> {
    loop {
        _ = wake_up_notify.notified();

        tracing::info!("Propagate root triggered");

        let is_unsynced = TaskMonitor::check_if_unsynced(&app.database).await?;

        if !is_unsynced {
            continue;
        }
        
        let tx_id = app.bridge_processor
            .propagate_root()
            .await?;

        monitored_txs_sender.send(tx_id.clone()).await?;

        // update db state to pending
        app.database.mark_status_as_pending().await?;

        tokio::time::sleep(app.config.app.time_between_scans).await;
    }
}
