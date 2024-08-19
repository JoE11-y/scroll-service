use chrono::{DateTime, Utc};
use sqlx::prelude::FromRow;
use serde::{Deserialize, Serialize};


#[derive(Debug, Clone, FromRow, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ServerStatus {
    pub status: String,
    pub last_synced: Option<DateTime<Utc>>,
}
