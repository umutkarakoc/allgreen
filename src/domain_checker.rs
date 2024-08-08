use crate::appconfig::ENV;
use chrono::{Utc, DateTime};
use chrono_humanize::{Humanize, HumanTime};
use futures::{stream, StreamExt};
use reqwest;
use sqlx::{Pool, Postgres};
use std::time::Duration;
use uuid::Uuid;
use scraper::{Html, Selector};

struct WebsiteFetch {
    id: Uuid,
    url: String
}

pub async fn checker(client: Pool<Postgres>, nullonly: bool) {
    let http = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .tcp_keepalive(None)
        .build()
        .unwrap();

    }
