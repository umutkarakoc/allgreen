use crate::appconfig::ENV;
use chrono::{DateTime, Days, Utc};
use futures::{stream, StreamExt};
use once_cell::sync::Lazy;
use reqwest;
use scraper::{Html, Selector};
use sqlx::{Pool, Postgres};
use std::{process::Stdio, time::Duration};

use execute::Execute;
use std::process::Command;

pub static HTTP: Lazy<reqwest::Client> = Lazy::new(|| {
    reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .tcp_keepalive(None)
        .build()
        .unwrap()
});

pub async fn check_state(url: String, keyword: String, useragent: Option<String>) -> (String, i64) {
    let useragent = match useragent {
        Some(ua) => format!("{} allgreen.me Up Time Checker", ua.clone() ),
        None => "Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/108.0.0.0 Safari/537.36 allgreen.me Up Time Checker".to_string()
    };

    let start = Utc::now().time();
    let resp = HTTP
        .get(&url)
        .header("User-Agent", &useragent)
        .timeout(Duration::from_secs(ENV.checker_timeout))
        .send()
        .await;

    match resp {
        Ok(body) => {
            let html = body.text().await;
            let state = match html {
                Ok(html) => {
                    if html.contains(keyword.as_str()) {
                        "up"
                    } else {
                        "down"
                    }
                }
                Err(_) => "down",
            };

            (
                state.to_string(),
                (Utc::now().time() - start).num_milliseconds(),
            )
        }
        Err(_) => ("down".to_string(), 0),
    }
}

pub async fn check_health(db: Pool<Postgres>) {
    loop {
        let websites = sqlx::query!(
            r#"select ws.id, ws.keyword, ws.url, ws.useragent, u.discord_webhook,
                last.state as state, ws.is_paused
            from website ws
            join (
                select 
                    *,
                    ROW_NUMBER() OVER(PARTITION BY website_id ORDER BY created_at desc) AS rn
                from website_state
                order by rn 
            ) as last on last.website_id = ws.id and last.rn = 1
            join "user" u on u.id = ws.user_id
             "#
        )
        .fetch_all(&db)
        .await
        .unwrap();

        stream::iter(websites)
            .map(|ws| {
                let client = db.clone();
                tokio::spawn(async move {
                    if ws.is_paused {
                        if (ws.is_paused && ws.state == "paused") {
                            return;
                        }
                        sqlx::query!(
                            r#"insert into website_state (website_id, state, duration)
                                values ($1, $2, $3)  "#,
                            ws.id,
                            "paused",
                            0
                        )
                        .execute(&client)
                        .await
                        .unwrap();
                        return;
                    }
                    let (mut state, mut duration) =
                        check_state(ws.url.clone(), ws.keyword.clone(), ws.useragent.clone()).await;

                    if state == "down" {
                        if duration > 10000 {
                            //recheck  if duration > 10seconds
                            (state, duration) =
                                check_state(ws.url.clone(), ws.keyword, ws.useragent.clone()).await;
                        }
                    };
                    println!("0");
                    if ws.state != state {
                        println!("1");
                        sqlx::query!(
                            r#"insert into website_state (website_id, state, duration)
                                values ($1, $2, $3)  "#,
                            ws.id,
                            state,
                            duration
                        )
                        .execute(&client)
                        .await
                        .unwrap();
                        println!("2");
                        println!("{} {} {} {:?}", ws.url, ws.state, state, ws.discord_webhook);
                        println!("test");
                        match &ws.discord_webhook {
                            Some(url) => {
                                let url = url.clone();
                                tokio::spawn(async move {
                                    let resp = HTTP
                                        .post(url)
                                        .header("content-type", "application/json")
                                        .body(format!(
                                            r#"{{ "content" : " {} is {} "  }}"#,
                                            ws.url, state
                                        ))
                                        .send()
                                        .await;

                                    if resp.is_err() {
                                        println!("error at url discord: {}", ws.url.clone());
                                    }
                                });
                            }
                            None => {}
                        };
                    }
                })
            })
            .buffer_unordered(ENV.checker_batch)
            .for_each(|_| async {})
            .await;

        tokio::time::sleep(Duration::from_secs(ENV.checker_sleep)).await;
        // thread::sleep(time::Duration::from_secs(ENV.checker_sleep));
    }
}

pub async fn check_domain(db: Pool<Postgres>) {
    loop {
        let today = Utc::now().checked_sub_days(Days::new(1));

        let websites = sqlx::query!(
            r#"select ws.id, ws.keyword, ws.url, ws.useragent, u.discord_webhook
            from website ws
            join "user" u on u.id = ws.user_id
            where 
                (last_domain_checked_at is null 
                    or domain_expire_at is null 
                    or  last_domain_checked_at <= $1) "#,
            today
        )
        .fetch_all(&db)
        .await
        .unwrap();

        for ws in websites.iter() {
            let url = ws.url.replace("https://", "");
            let url = url.replace("http://", "");
            let url: Vec<&str> = url.split("/").collect();
            let url = url.first().unwrap();

            println!("url: {}", url);

            let url = format!("https://www.whois.com/whois/{}", &url);
            let resp = HTTP
                .get(&url)
                .timeout(Duration::from_secs(ENV.checker_timeout))
                .send()
                .await;
            let Ok(body) = resp else {
                continue;
            };

            let html = body.text().await.unwrap();
            let expire_at = {
                let document = Html::parse_document(&html);

                if url.ends_with(".tr") {
                    if let Ok(selector) = Selector::parse("#registryData") {
                        let txt = document.select(&selector).nth(0);
                        if let Some(txt) = txt {
                            let txt = txt.text().collect::<Vec<&str>>().join("");
                            let index = txt.find("Expires on").unwrap_or(0);
                            let txt = &txt[index + 26..index + 37];
                            println!("{} {}", url, txt);
                            let expire_at = format!("{} 00:00:00 +00:00", txt);
                            match DateTime::parse_from_str(&expire_at, "%Y-%b-%d %H:%M:%S %z") {
                                Ok(date) => Some(date),
                                Err(err) => {
                                    println!("err: {} ", err);
                                    None
                                }
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    if let Ok(selector) = Selector::parse(".df-block .df-value") {
                        let expire_at = document.select(&selector).nth(3);

                        if let Some(expire_at) = expire_at {
                            let expire_at = expire_at.text().collect::<Vec<&str>>().join("");
                            let expire_at = format!("{} 00:00:00 +00:00", expire_at);
                            match DateTime::parse_from_str(&expire_at, "%Y-%m-%d %H:%M:%S %z") {
                                Ok(date) => Some(date),
                                Err(err) => {
                                    println!("err: {} ", err);
                                    None
                                }
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
            };

            sqlx::query!(
                r#"update website set domain_expire_at = $2, last_domain_checked_at = $3 where id = $1"#,
                ws.id,
                expire_at,
                Some(Utc::now())
            )
            .execute(&db)
            .await
            .unwrap();
            tokio::time::sleep(Duration::from_secs(15)).await;
        }

        tokio::time::sleep(Duration::from_secs(60)).await;
    }
}

pub async fn check_ssl(db: Pool<Postgres>) {
    loop {
        let today = Utc::now().checked_sub_days(Days::new(1));

        let websites = sqlx::query!(
            r#"select ws.id, ws.keyword, ws.url, ws.useragent, u.discord_webhook
            from website ws
            join "user" u on u.id = ws.user_id
            where 
                (last_domain_checked_at is null 
                    or ssl_expire_at is null 
                    or last_ssl_checked_at <= $1) "#,
            today
        )
        .fetch_all(&db)
        .await
        .unwrap();

        for ws in websites.iter() {
            let mut cmd = Command::new("nmap");

            let url = ws.url.replace("https://", "");
            let url: Vec<&str> = url.split("/").collect();
            let url = url.first().unwrap();

            cmd.arg("-p")
                .arg("443")
                .arg("--script")
                .arg("ssl-cert")
                .arg(url);

            cmd.stdout(Stdio::piped());

            let Ok(output) = cmd.execute_output() else {
                println!("{} nmap error", url);
                continue;
            };
            let Ok(output) = String::from_utf8(output.stdout) else {
                println!("{} cant read output", url);
                continue;
            };

            let not_valid_after: Vec<&str> = output
                .split("\n")
                .filter(|l| l.contains("Not valid after"))
                .collect();
            let Some(not_valid_after) = not_valid_after.first() else {
                println!("{} cant read date", url);
                continue;
            };
            let not_valid_after = not_valid_after.replace("| Not valid after:", "");
            let not_valid_after = not_valid_after.trim().replace("T", " ").to_string();
            let not_valid_after = format!("{}  +00:00", not_valid_after);
            let not_valid_after =
                DateTime::parse_from_str(&not_valid_after, "%Y-%m-%d %H:%M:%S %z");
            let Ok(expire_at) = not_valid_after else {
                println!("{} cant parse date", url);
                continue;
            };

            sqlx::query!(
                r#"update website set  ssl_expire_at = $2 where id = $1"#,
                ws.id,
                expire_at
            )
            .execute(&db)
            .await
            .unwrap();
            tokio::time::sleep(Duration::from_secs(5)).await;
        }

        tokio::time::sleep(Duration::from_secs(60)).await;
    }
}

pub async fn run(db: Pool<Postgres>) {
    let db1 = db.clone();
    let db2 = db.clone();
    let db3 = db.clone();

    tokio::spawn(async move { check_health(db1).await });
    tokio::spawn(async move { check_domain(db2).await });
    tokio::spawn(async move { check_ssl(db3).await });
}
