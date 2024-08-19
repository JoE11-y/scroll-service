use std::sync::Arc;
use std::time::Duration;

use crate::task_monitor::{App, TaskMonitor};
use tokio::sync::Notify;
use tokio::time;
use tracing::info;

pub async fn check_sync(app: Arc<App>, wake_up_notify: Arc<Notify>) -> anyhow::Result<()> {
    let mut timer = time::interval(Duration::from_secs(600));
    loop {
        _ = timer.tick().await;
        info!("Sync processor woken due to timeout.");

        let is_synced = TaskMonitor::check_synced_state(&app).await?;

        let is_pending = TaskMonitor::check_if_propagated(&app.database).await?;

        // if still synced or pending continue so as not to call the propagate function twice
        if is_synced && is_pending {
          app.database.mark_status_as_synced().await?;
          continue;
        } else if is_synced || is_pending {
          continue;
        }

        app.database.mark_status_as_unsynced().await?;

        // else notify the propagation task
        wake_up_notify.notify_one()
    }
}
