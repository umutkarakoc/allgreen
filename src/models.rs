use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Deserialize, Serialize, Default)]
pub struct WebSite {
    pub id: Uuid,
    pub url: String,
    pub keyword: String,
    pub tags: String,
    pub created_at: DateTime<Utc>,
    pub domain_expire_at: Option<DateTime<Utc>>,
    pub user_id: Uuid,
    pub useragent: Option<String>,
    pub last_domain_checked_at: Option<DateTime<Utc>>,
    pub ssl_expire_at: Option<DateTime<Utc>>,
}

#[derive(Deserialize, Serialize, Default)]
pub struct WebSiteState {
    pub id: Uuid,
    pub state: bool,
    pub created_at: DateTime<Utc>,
    pub website_id: Uuid,
}

#[derive(Deserialize, Serialize)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub registered_at: Option<DateTime<Utc>>,
    pub discord_webhook: Option<String>,
}

#[derive(Deserialize, Serialize)]
pub struct LoginCode {
    pub id: Uuid,
    pub email: String,
    pub created_at: DateTime<Utc>,
    pub state: String,
    pub code: String,
}
