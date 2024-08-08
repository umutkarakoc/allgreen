use super::layout;
use crate::{logged_user::LoggedUser, AppState};
use axum::http::HeaderMap;
use axum::routing::get;
use axum::{extract::*, response::*, Router, http::StatusCode};
use chrono::{Duration, Utc, DateTime};
use chrono_humanize::HumanTime;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::ops::Add;
use uuid::Uuid;
use maud::{html, Markup};
const DAY: i64 = 24 * 60 * 60;

// #[derive(Default)]
// pub struct WebsiteRow {
//     pub id: String,
//     pub url: String,
//     pub tags: String,
//     pub keyword: String,
//     pub state: String,
//     pub last_checked: String,
//     pub paused: bool,
//     pub last_changed: String,
//     pub last24_bar: Vec<(String, &'static str)>, // len, state, previus state
// }

#[derive(Deserialize, Default)]
pub struct HomeQuery {
    pub search: Option<String>,
    pub state: Option<String>,
    pub page: Option<i64>,
    pub is_paused: Option<bool>
}

#[derive(Serialize, Default)]
pub struct  SummaryRow {
    pub id: Uuid,
    pub url: String,
    pub created_at: DateTime<Utc>,
    pub domain_expire_at: Option<DateTime<Utc>>,
    pub last_domain_checked_at: Option<DateTime<Utc>>,
    pub ssl_expire_at: Option<DateTime<Utc>>,
    pub state : String,
    pub last_checked_at: DateTime<Utc>,
    pub is_paused: bool
}

pub async fn home_get(
    LoggedUser(user_id): LoggedUser, 
    State(db): State<PgPool>, 
    Query(query): Query<HomeQuery>
) -> impl IntoResponse {

    let page = query.page.unwrap_or(1);
    let page_size = 30;

    let count = sqlx::query!(
        r#"select count(*) from website where user_id = $1 "#,
        user_id
    ).fetch_one(&db)
        .await.unwrap();
    let count = count.count.unwrap_or(0);
    let pages = (count as f32 / page_size as f32).ceil() as i32;

    let websites = sqlx::query_as!(
        SummaryRow,
        r#"select 
            ws.id, 
            ws.url,
            ws.created_at,
            ws.domain_expire_at,
            ws.last_domain_checked_at,
            ws.ssl_expire_at,
            ws.is_paused,
            last.state as state,
            last.created_at as last_checked_at
        from website as ws
        join (
            select 
                *,
                ROW_NUMBER() OVER(PARTITION BY website_id ORDER BY created_at desc) AS rn
            from website_state
            order by rn 
        ) as last on last.website_id = ws.id and last.rn = 1
        where ws.user_id = $1 and ( url like $3 or tags like $3 or $3 is null )
        order by last.state, is_paused
        offset $2 limit $4"#,
        user_id,
        (page - 1) * page_size,
        query.search.map(|s| format!("%{}%", s)),
        page_size
    )
    .fetch_all(&db)
        .await.unwrap();


    let up = websites.iter().filter(|w| w.state == "up" ).count();
    let down = websites.iter().filter(|w| w.state == "down").count();
    let paused = websites.iter().filter(|w| w.is_paused).count();

    let now = Utc::now();
    let yesterday = now.add(Duration::days(-1));

    let websites = if let Some(state) = query.state {
        websites.into_iter().filter(|w| w.state == state).collect::<Vec<SummaryRow>>()
    } else {
        websites
    };

    let websites = if let Some(state) = query.is_paused {
        websites.into_iter().filter(|w| w.is_paused).collect::<Vec<SummaryRow>>()
    } else {
        websites
    };

    let ids = &websites.iter().map(|ws| ws.id).collect::<Vec<Uuid>>()[..];
    let last24_states = sqlx::query!(
        r#"select * from "website_state" where website_id = any($1) order by created_at desc"#,
        ids
    )
    .fetch_all(&db)
    .await.unwrap();

    let header = html!{
        header style="display: flex; justify-content: space-between; align-items:center"
                class="media" {

                a class="up" href="/" hx-boost="true" hx-target="body" hx-swap="outerHTML" {
                    img src="/logofull.png" style="height:60px" {}
                }

                div style="display:flex;" {
                    input type="text" required name="search" id="search" placeholder="search"
                        class="input is-rounded"
                        hx-swap="multi:#websites,#summary"  hx-get="/" hx-push-url="true" {}

                    button hx-get="/website/create" hx-swap="multi:#modal:outerHTML" 
                        class="button is-primary is-inverted ml-1"
                        data-tooltip="Add Website" data-placement="bottom" {
                        span class="icon" { i class="fa fa-plus" {} }
                    }
                    button class="button ml-1 is-black is-inverted" 
                        hx-get="/setting"  hx-swap="multi:#modal:outerHTML"                         data-tooltip="Setting" data-placement="bottom" {
                        span class="icon" {i class="fa-solid fa-gear" {}}
                    }
                    button class="button ml-1 is-black is-inverted" 
                        hx-post="/auth/logout" hx-target="body" hx-swap="outherHTML" hx-push-url="true"
                        data-tooltip="Logout" data-placement="bottom" {
                        span class="icon" {i class="fa-solid fa-right-from-bracket has-text-danger" {}}
                    }

                }
            }
        };

    let websites = if websites.len() == 0 {
            html!{
                div class="flex1 is-flex is-justify-content-center is-align-items-center" {
                    div class="is-flex is-flex-direction-column is-justify-content-center " {
                        p class="subtitle" {"There is no websites to monitor"}
                        button hx-get="/website/create" hx-swap="multi:#modal:outerHTML" 
                            class="button is-primary" {
                            span {"Add Website"}
                        }
                    }
                }
            }   
        } else {
            let rows: Vec<Markup> = websites
                .iter()
                .map(|ws| {
                    let mut last24_bar: Vec<(String, &'static str)> = vec![];

                    let history = last24_states.iter().filter(|s| s.website_id == ws.id);

                    let mut dt = now;
                    for s in history {
                        let state = match s.state.as_str() {
                            "up" => "has-background-primary",
                            "down" =>  "has-background-danger",
                            _ => "has-background-dark"
                        };

                        if s.created_at >= yesterday {
                            let len = dt.timestamp() - s.created_at.timestamp();
                            let percent = format!("{:2}", ((len as f64 / DAY as f64) * 100f64));

                            last24_bar.push((percent, state));
                        }

                        if s.created_at < yesterday && dt > yesterday {
                            let len = dt.timestamp() - yesterday.timestamp();
                            let percent = format!("{:2}", ((len as f64 / DAY as f64) * 100f64));

                            last24_bar.push((percent, state));
                        }

                        dt = s.created_at;
                    }

                    let state_color =
                        if ws.is_paused {
                            "dark".to_string()
                        }
                        else if ws.state == "up" {
                            "primary".to_string()
                        } else if ws.state == "down" {
                            "danger".to_string()
                        }
                        else {
                            "dark".to_string()
                        };

                    let is_paused = if ws.is_paused { "paused" } else { "not paused" };
                    let state_text = ws.state.clone() ;

                    let domain_state = if let Some(expire_at) = ws.domain_expire_at {
                        let days = (expire_at - Utc::now()).num_days();
                        let color = if days < 5 { "has-text-danger" } 
                            else if days < 30 { "has-text-warning" }
                            else { "has-text-dark" };
                        html!{td class=(color) { 
                            (expire_at.format("%d/%m/%Y").to_string()) 
                        }}
                    } else {
                        html!{ td { "-" }}
                    };

                     let ssl_state = if let Some(expire_at) = ws.ssl_expire_at {
                        let days = (expire_at - Utc::now()).num_days();
                        let color = if days < 5 { "has-text-danger" } 
                            else if days < 30 { "has-text-warning" }
                            else { "has-text-dark" };
                        html!{td class=(color) { 
                            (expire_at.format("%d/%m/%Y").to_string()) 
                        }}
                    } else {
                        html!{ td { "-" }}
                    };

                    html!{tr {
                        td {
                            a class={"subtitle m-1 has-text-"(state_color)} href={"/website/"(ws.id.to_string())} { ( ws.url )}
                        }
                        td class={"subtitle is-size-6 m-1 has-text-dark"} { (is_paused) }
                        td class={"subtitle is-size-6 m-1 has-text-"(state_color)} { (state_text) }
                        (domain_state)
                        (ssl_state)
                        td { (HumanTime::from(ws.last_checked_at).to_string() ) }
                        td data-tooltip="Last 24 hours records" data-placement="bottom" {
                            div style="width: calc(100% - 10px);  position: relative;"
                               class="is-flex cursor is-flex-direction-row-reverse has-background-dark"  {
                                @for (len,state) in last24_bar.iter() { 
                                    div style={"width: " (len) "%; height:30px"} class={"bar has-background-dark "(state)} {}
                                }
                            }
                        }
                        td class="hidden-mobile" {
                            div class="is-flex" {
                                button class="button is-danger is-inverted is-small" 
                                    style="width: 40px; margin:0; margin-left: 5px"
                                    hx-delete={"/website/" (ws.id.to_string())} hx-confirm="Do you want to delete this website?"
                                    hx-swap="multi:#websites:outerHTML" {
                                        i class="fa fa-trash" {}
                                    }
                                button class={"button is-light is-small is-dark is-inverted" }
                                    style="width: 40px; margin:0; margin-left: 5px"
                                    hx-post={"/website/" (ws.id.to_string()) (if ws.is_paused { "/play" } else { "/pause" })}
                                    hx-swap="multi:#websites:outerHTML"
                                    aria-label={ (if ws.is_paused { "Start" } else { "Pause"})" website monitor"} data-cooltipz-dir="bottom" {
                                        i class={"fa fa-" (if ws.is_paused { "play" } else { "pause" })} {}
                                }
                            }
                        }
                    }}
                })
                .collect();
            
            html!{
                table class="table" {
                    thead {
                        tr {
                            th scope="col" {"Website"}
                            th scope="col" class="hidden-desktop" style="width: 80px;"{"Is Paused"}
                            th scope="col" class="hidden-desktop" style="width: 80px;"{"Last State"}
                            th scope="col" style="width: 150px;"{"Domain Expire"}
                            th scope="col" style="width: 150px;"{"SSL Expire"}
                            th scope="col" style="width: 150px;"{"Changed"}
                            th scope="col" class="hidden-desktop" style="width: 150px;"{"Checked"}
                            th scope="col" style="width:200px"{"Last 24h"}
                            th scope="col" class="hidden-mobile" {}
                        }
                    }
                    tbody {
                        @for row in rows { (row) }
                    }
                }

                div class="pagination" role="navigation" aria-label="pagination" {
                    ul class="pagination-list" {
                        @for i in 1..pages{
                            li {
                                a  class={"pagination-link " (if (i == 1 && query.page.is_none() ) || query.page == Some(i as i64) {"is-current"} else {""})}
                                    href={"?page="(i)} {
                                    (i)
                                }
                            }
                        }
                    }
                }
            }
        };

    let stats = html!{ 
        div class="is-flex h-6 mt-4" {
            a class="card has-background-primary flex1 mr-1 p-6 is-flex is-flex-direction-column is-justify-content-center is-align-items-center"
                href="?state=up" {
                span class="title is-size-1  has-text-white " { (up ) }
                span class="subtitle is-size-5  has-text-grey has-text-grey-lighter " {  "up" }
            }
            a class="card has-background-danger has-text-white flex1 ml-1 mr-1 p-6 is-flex is-flex-direction-column is-justify-content-center is-align-items-center"
                href="?state=down" {
                span class="title is-size-1  has-text-white " { (down ) }
                span class="subtitle is-size-5  has-text-grey has-text-grey-lighter " {  "down/timeout" }
            }
            a class="card has-background-dark has-text-white flex1 ml-1 p-6 is-flex is-flex-direction-column is-justify-content-center is-align-items-center"
                href="?is_paused=true" {
                span class="title is-size-1  has-text-white " { (paused ) }
                span class="subtitle is-size-5 has-text-grey-lighter" {  "pause/nodata" }
            }

        }

    };
    layout::page(html!{}, html! { 
            div class="has-background-white p-5 is-flex is-flex-direction-row" style="min-height: 100vh"
            hx-trigger="every 10s" hx-get="" hx-swap="multi:#websites:outerHTML,#summary:outerHTML"   {
                div class="container is-flex is-flex-direction-column"  {

                    (header)

                    (stats)

                    div id="websites" class="is-flex flex1 is-flex-direction-column mt-6" {
                        (websites)
                    }
                }
            }
            div id="modal"{}
        }).into_response()

}

pub async fn test() -> impl IntoResponse {
    layout::page(html!{}, 
        html!{
            div {
                "hello"
            }
        })
}

pub async fn get_setting(
    State(db): State<PgPool>,
    LoggedUser(user_id): LoggedUser,
) -> impl IntoResponse {
    let user = sqlx::query!(r#"select * from "user" where id = $1"#, user_id)
        .fetch_one(&db)
        .await
        .unwrap();


    let form = html!{
        form hx-target="#error"  hx-post="/setting" class="box" {
            h2 class="title" { "Monitor A Website" }
            div class="field" {
                label class="label" {"Name"}
                div class="control has-icons-left" {
                    input class="input" name="name" placeholder="name" required
                    value=(user.name)  {}
                    span class="icon is-small is-left" {i class="fa-solid fa-link" {} }
                }
            }

            div class="field" {
                label class="label" {"E-mail"}
                div class="control has-icons-left" {
                    input class="input" placeholder="email" name="email" value=(user.email) {}
                    span class="icon is-tags is-left" {i class="fa-solid fa-magnifying-glass" {} }
                }
            }

            div class="field" {
                label class="label" {"Discord Webgook for Notification"}
                div class="control has-icons-left" {
                    input class="input" placeholder="url" name="discord_webhook" value=(user.discord_webhook.unwrap_or("".to_string()))  {}
                    span class="icon is-tags is-left" {i class="fa-solid fa-tags" {} }
                }
            }

            div style="width: 100%; display: flex; justify-content: space-between;"{
                div {
                    button type="submit" class="button is-primary" {
                        "Save"
                    }
                }

                div {
                    button class="button is-danger is-light" hx-get="" 
                    hx-target="body" hx-swap="outherHTML"
                    hx-indicator="button"
                    style="width:100px" {
                        "Cancel"
                    }
                }
            }

            div style=" margin-top: 10px" {
                span class="down" id="error" {}
            }
        }
    };

    (html!{
        div class="modal is-active" id="modal" {
            div class="modal-background" {}
            div class="modal-content" {
                (form)
            }
            button class="modal-close is-large" aria-label="close" {}
        }
    }).into_string()
}

#[derive(Deserialize)]
pub struct UpdateSettingParam {
    pub name: String,
    pub email: String,
    pub discord_webhook: String,
}
pub async fn post_setting(
    State(client): State<PgPool>,
    LoggedUser(user_id): LoggedUser,
    Form(params): Form<UpdateSettingParam>,
) -> impl IntoResponse {
    let wb = if params.discord_webhook.is_empty() {
        None
    } else {
        Some(params.discord_webhook)
    };
    sqlx::query!(
        r#"update "user" set name=$3, discord_webhook=$2 where id = $1"#,
        user_id,
        wb,
        params.name
    )
    .execute(&client)
    .await
    .unwrap();

     let mut headers = HeaderMap::new();
    headers.insert("HX-Refresh", "true".parse().unwrap());
    (StatusCode::FOUND, headers).into_response()
}


pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(home_get))
        .route("/setting", get(get_setting).post(post_setting))
        .route("/test", get(test))
}

