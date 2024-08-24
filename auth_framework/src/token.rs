use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct DashboardClaims {
    pub id: Uuid,
    pub sid: Uuid,
}
