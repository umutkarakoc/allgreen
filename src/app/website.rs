use crate::logged_user::LoggedUser;
use crate::{AppState, checker};
use axum::http::HeaderMap;
use axum::routing::{get, post};
use axum::{extract::*, response::*, Router, http::StatusCode};
use chrono::{Duration, Utc};
use maud::{html, Markup};
use serde::Deserialize;
use sqlx::PgPool;
use std::ops::Add;
use uuid::Uuid;

use super::layout;

pub const DAY: i64 = 24 * 60 * 60;
pub const WEEK: i64 = 24 * 60 * 60 * 7;
pub const YEAR: i64 = 24 * 60 * 60 * 365;

async fn get_website_create(
    LoggedUser(_): LoggedUser
) -> impl IntoResponse {

    let form = html!{
        form hx-target="#error"  hx-post="/website" class="box" {
            h2 class="title" { "Monitor A Website" }
            div class="field" {
                label class="label" {"Url to monitor"}
                div class="control has-icons-left" {
                    input class="input" type="url" name="url" placeholder="Url" required
                    value="https://yourwebsite.com/"  {}
                    span class="icon is-small is-left" {i class="fa-solid fa-link" {} }
                }
            }

            div class="field" {
                label class="label" {"Keyword"}
                div class="control has-icons-left" {
                    input class="input" placeholder="Keyword" name="keyword" value="" required  {}
                    span class="icon is-tags is-left" {i class="fa-solid fa-magnifying-glass" {} }
                }
            }

            div class="field" {
                label class="label" {"Tags"}
                div class="control has-icons-left" {
                    input class="input" placeholder="Tags" name="tags" value=""  {}
                    span class="icon is-tags is-left" {i class="fa-solid fa-tags" {} }
                }
            }

            div class="field" {
                label class="label" {"UserAgent"}
                div class="control has-icons-left" {
                    input class="input" placeholder="UserAgent" name="useragent" value=""  {}
                    span class="icon is-tags is-left" {i class="fa-brands fa-chrome" {} }
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


pub async fn get_website(
    State(db): State<PgPool>,
    LoggedUser(_): LoggedUser,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    let ws = sqlx::query!(r#"
        select ws.* , last.state as state, last.created_at as last_checked_at
        from website as ws 
        join (
            select 
                *,
                ROW_NUMBER() OVER(PARTITION BY website_id ORDER BY created_at desc) AS rn
            from website_state
            order by rn 
        ) as last on last.website_id = ws.id and last.rn = 1
        where ws.id = $1 "#, id)
        .fetch_one(&db)
        .await
        .unwrap();
    let now = Utc::now();
    let yesterday = now.add(Duration::days(-1));
    let lastweek = now.add(Duration::days(-7));
    let lastyear = now.add(Duration::days(-365));


    let history = sqlx::query!(
        r#"select * from website_state
            where website_id = $1 
            order by created_at desc"#,
        id
    )
    .fetch_all(&db)
    .await
    .unwrap();

    let mut last24_bar: Vec<Markup> = vec![];
    let mut lastweek_bar: Vec<Markup> = vec![];
    let mut lastyear_bar: Vec<Markup> = vec![];
    let mut last24_uptime = 0;
    let mut lastweek_uptime = 0;
    let mut lastyear_uptime = 0;

    let mut dt = now;
    for s in history.iter() {
        let state_text = s.state.clone();
        let state_color = 
            if s.state == "up" { 
                "has-background-primary" 
            } else { 
                "has-background-danger" 
            };

        if s.created_at >= yesterday {
            let len = dt.timestamp() - s.created_at.timestamp();
            let percent = format!("{:2}", ((len as f64 / DAY as f64) * 100f64));

            last24_bar.push(html!{div style={"height:30px; width:"(percent)"%"}
                aria-label=(state_text) data-cooltipz-dir="bottom"
                class=(state_color)  {}
            });

            if s.state == "up" {
                last24_uptime += len;
            }
        }
        if s.created_at >= lastweek {
            let len = dt.timestamp() - s.created_at.timestamp();
            let percent = format!("{:2}", ((len as f64 / WEEK as f64) * 100f64));

            lastweek_bar.push(html!{div style={"height:30px; width:"(percent)"%"}
                aria-label=(state_text) data-cooltipz-dir="bottom"
                class=(state_color)  {}
            });


            if s.state == "up" {
                lastweek_uptime += len;
            }
        }
        if s.created_at >= lastyear {
            let len = dt.timestamp() - s.created_at.timestamp();
            let percent = format!("{:2}", ((len as f64 / YEAR as f64) * 100f64));

            lastyear_bar.push(html!{div style={"height:30px; width:"(percent)"%"}
                aria-label=(state_text) data-cooltipz-dir="bottom"
                class=(state_color)  {}
            });


            if s.state == "up" {
                lastyear_uptime += len;
            }
        }

        if s.created_at < yesterday && dt > yesterday {
            let len = dt.timestamp() - yesterday.timestamp();
            let percent = format!("{:2}", ((len as f64 / DAY as f64) * 100f64));

            last24_bar.push(html!{div style={"height:30px; width:"(percent)"%"}
                aria-label=(state_text) data-cooltipz-dir="bottom"
                class=(state_color)  {}
            });


            if s.state == "up" {
                last24_uptime += len;
            }
        }
        if s.created_at < lastweek && dt > lastweek {
            let len = dt.timestamp() - lastweek.timestamp();
            let percent = format!("{:2}", ((len as f64 / WEEK as f64) * 100f64));

            lastweek_bar.push(html!{div style={"height:30px; width:"(percent)"%"}
                aria-label=(state_text) data-cooltipz-dir="bottom"
                class=(state_color)  {}
            });


            if s.state == "up" {
                lastweek_uptime += len;
            }
        }

        if s.created_at < lastyear && dt > lastyear {
            let len = dt.timestamp() - lastyear.timestamp();
            let percent = format!("{:2}", ((len as f64 / YEAR as f64) * 100f64));

            lastweek_bar.push(html!{div style={"height:30px; width:"(percent)"%"}
                aria-label=(state_text) data-cooltipz-dir="bottom"
                class=(state_color)  {}
            });


            if s.state == "up" {
                lastyear_uptime += len;
            }
        }

        dt = s.created_at;
    }

    let last = history.last();
    if ws.created_at > yesterday && last.is_some() {
        let last = last.unwrap();
        last24_uptime += (last.created_at - yesterday).num_seconds();
    }
    if ws.created_at > lastweek && last.is_some() {
        let last = last.unwrap();
        lastweek_uptime += (last.created_at - lastweek).num_seconds();
    }
    if ws.created_at > lastyear && last.is_some() {
        let last = last.unwrap();
        lastyear_uptime += (last.created_at - lastyear).num_seconds();
    }

    let last24_uptime = format!("{:.2}%", last24_uptime as f64 / DAY as f64 * 100f64);
    let lastweek_uptime = format!("{:.2}%", lastweek_uptime as f64 / WEEK as f64 * 100f64);
    let lastyear_uptime = format!("{:.2}%", lastyear_uptime as f64 / YEAR as f64 * 100f64);

    let last_state_color = history.first()
        .map(|x| match x.state.as_str() { 
            "up" => "has-text-primary",
            "down" => "has-text-danger",
            _ => "has-text-dark"
        })
        .unwrap_or("has-text-dark");
    let last_state_text = history.first().unwrap().state.clone();

    let last_checked =  format!("ed at {}" , ws.last_checked_at.format("%d/%m/%Y %H:%M").to_string() ) ;


    let form =  html!{
        div class="flex1" {
            form class="box m-0 mr-2" hx-target="#error"  hx-put={"/website/"(ws.id.to_string())} {
                div class="field" {
                    label class="label" {"Url"}
                    div class="control has-icons-left" {
                        input class="input" type="url" name="url" placeholder="Url" 
                        value=(ws.url)  {}
                        span class="icon is-small is-left" {i class="fa-solid fa-link" {} }
                    }
                }

                div class="field" {
                    label class="label" {"Keyword"}
                    div class="control has-icons-left" {
                        input class="input" placeholder="Keyword" name="keyword" value=(ws.keyword)  {}
                        span class="icon is-tags is-left" {i class="fa-solid fa-magnifying-glass" {} }
                    }
                }

                div class="field" {
                    label class="label" {"Tags"}
                    div class="control has-icons-left" {
                        input class="input" placeholder="Tags" name="tags" value=(ws.tags)  {}
                        span class="icon is-tags is-left" {i class="fa-solid fa-tags" {} }
                    }
                }

                div class="field" {
                    label class="label" {"UserAgent"}
                    div class="control has-icons-left" {
                        input class="input" placeholder="UserAgent" name="useragent" 
                            value=(ws.useragent.unwrap_or("".to_string()))  {}
                        span class="icon is-tags is-left" {i class="fa-brands fa-chrome" {} }
                    }
                }

                div style="width: 100%; display: flex; justify-content: space-between;"{
                    div {
                        button type="submit" class="button is-primary" {
                            "Save"
                        }
                    }

                    div {
                        button class="button is-danger is-light" hx-delete={"/website/"(ws.id.to_string())} 
                        hx-target="body" hx-confirm="true"
                        style="width:100px" {
                            "Delete"
                        }
                    }
                }

                div style=" margin-top: 10px" {
                    span class="down" id="error" {}
                }
            }
        }
    };

    let bars = html!{ div class="box" {
        div style="display: flex; justify-content: space-between;" class="mt-2"{
            span {"Last Day"}
            span { (last24_uptime) "uptime" }
        }
        div style="width: 100%; position: relative;"  
            class="is-flex is-flex-direction-row-reverse has-background-dark"{
            @for bar in last24_bar.iter() {
                (bar)
            }
        }

        div style="display: flex; justify-content: space-between;" class="mt-2"{
            span {"Last Week"}
            span { (lastweek_uptime) "uptime" }
        }
        div style="width: 100%; position: relative;"  
            class="is-flex is-flex-direction-row-reverse  has-background-dark" {
            @for bar in lastweek_bar.iter() {
                (bar)
            }
        }

        div style="display: flex; justify-content: space-between;" class="mt-2" {
            span {"Last Year"}
            span { (lastyear_uptime) "uptime" }
        }
        div style="width: 100%; position: relative;"  
            class="is-flex is-flex-direction-row-reverse  has-background-dark" {
            @for bar in lastyear_bar.iter() {
                (bar)
            }
        }
    }};

    let history_table = html! { div class="box" style="width: 100%" {
        h6 class="title is-size-5" { "State History" }
        table class="table" style="width: 100%" {
            thead {
                tr {
                    th {"State"}
                    th {"Checked"}
                }
            }
            tbody {
                @for h in history.iter().take(10) {
                    tr {
                        td {
                            span class={(
                                match h.state.as_str() { 
                                    "up" => "has-text-primary",
                                    "down" =>  "has-text-danger",
                                    _ => "has-text-darl"  
                                })
                            } {
                                (h.state)
                            }
                        }
                        td {
                            (h.created_at.format("%H:%M %d/%m/%Y").to_string())
                        }
                    }
                }
            }
        }
    }};

    layout::page(html!{}, html! { 
        div class="has-background-white p-5 is-flex is-flex-direction-row" style="min-height: 100vh"
            hx-trigger="every 10s" hx-get="" hx-swap="multi:#websites:outerHTML,#summary:outerHTML"   {
                div class="container is-flex is-flex-direction-column"  {

                header style="display: flex; justify-content: space-between; align-items:center" {

                    a class="up" href="/" hx-boost="true" hx-target="body" hx-swap="outerHTML" {
                        img src="/logofull.png" style="height:60px" {}
                    }

                    div style="display:flex;" {
                        button class="button is-dark is-inverted ml-1" 
                            hx-get="/setting" hx-target="#modal"
                            data-tooltip="Setting" data-placement="bottom" {
                            span class="icon"{ i class="fa-solid fa-gear" {} }
                        }
                        button class="button is-danger is-inverted ml-1" 
                            hx-post="/auth/logout" hx-target="body" hx-push-url="true"
                            data-tooltip="Logout" data-placement="bottom" {
                            span class="icon"{ i class="fa-solid fa-right-from-bracket" {} }
                        }

                    }

                }

                div class="is-flex mt-4 is-justify-content-space-between is-align-items-center" {
                    h4 class={"title m-0 is-size-4 " (last_state_color) } { 
                       a href=(ws.url) target="_blank" class=(last_state_color){
                             (ws.url)            
                       }
                       " is " (last_state_text)
                    }

                    h5 class="title m-0 is-size-6" {
                        (last_checked)
                    }

                }

                div class="is-flex flex1 mt-2" {
                    ( form )

                    div class="is-flex flex1 ml-2 is-flex-direction-column" {
                        @if history.len() == 0 {
                            div class="box is-flex flex1 is-justify-content-center is-align-items-center" 
                            style="width: 100%; min-height: 300px" {
                                h3 class="subtitle" { "No data yet" }
                            }
                        } @else {
                            ( bars )
                            ( history_table )
                        }
                    }
                }
            }
        }

    }).into_response()
}


#[derive(Deserialize, Default)]
pub struct WebSiteUpdateParams {
    pub url: String,
    pub keyword: String,
    pub tags: String,
    pub useragent: String,
}

pub async fn update(
    LoggedUser(user_id): LoggedUser,
    State(db): State<PgPool>,
    Path(id): Path<Uuid>,
    Form(params): Form<WebSiteUpdateParams>,
) -> impl IntoResponse {
    let useragent = if params.useragent.len() == 0 {
        None
    } else {
        Some(params.useragent)
    };

    let update = sqlx::query_as!(
        Website,
        r#"update website set url = $1, keyword=$2, tags = $3 where id = $4 and user_id = $5"#,
        params.url,
        params.keyword,
        params.tags,
        id,
        user_id
    )
    .execute(&db)
    .await;

    // let (state, duration) = checker::check_state(params.url, params.keyword, useragent).await;

    //     sqlx::query!(
    //         r#"insert into website_state (website_id, state, duration)
    //             values ($1, $2, $3 )  "#,
    //         id,
    //         state,
    //         duration
    //     )
    //     .execute(&db)
    //     .await.unwrap();

    match update {
        Ok(_) => "".into_response(),
        Err(_) => "Website with this url already exits".into_response(),
    }
}


pub async fn create(
    State(db): State<PgPool>,
    LoggedUser(user_id): LoggedUser,
    Form(params) : Form<WebSiteUpdateParams>
) -> impl IntoResponse {

    let useragent = if params.useragent.len() == 0 {
        None
    } else {
        Some(params.useragent)
    };
    let website = sqlx::query!(r#"
        insert into website(user_id, keyword, url, tags , useragent) 
        values ($1, $2, $3, $4, $5) returning *"#, 
            user_id, params.keyword, params.url, params.tags, useragent )
        .fetch_one(&db).await
        .unwrap();

    sqlx::query!(
        r#"insert into website_state (website_id, state)
            values ($1, $2)  "#,
        website.id,
        "created"
    )
    .execute(&db)
    .await
    .unwrap();

    let mut headers = HeaderMap::new();
    headers.insert("hx-redirect", "/".parse().unwrap());
    headers.into_response()
}

pub async fn delete(
    State(client): State<PgPool>,
    LoggedUser(user_id): LoggedUser,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    let website = sqlx::query!(
        r#"select * from website where id = $1 and user_id = $2"#,
        id,
        user_id
    )
    .fetch_one(&client)
    .await;

    if website.is_ok() {
        sqlx::query!(r#"delete from website_state where website_id = $1"#, id)
            .execute(&client)
            .await
            .unwrap();

        sqlx::query!(
            r#"delete from website where user_id = $1 and id = $2"#,
            user_id,
            id
        )
        .execute(&client)
        .await
        .unwrap();
    }

    let mut headers = HeaderMap::new();
    headers.insert("hx-location", "/".parse().unwrap());

    return (StatusCode::SEE_OTHER, headers).into_response();
}

pub async fn pause(
    State(client): State<PgPool>,
    LoggedUser(_user_id): LoggedUser,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    
    let _ = sqlx::query!(
        r#"update website set is_paused=true
            where id = $1  "#,
        id
    )
    .execute(&client)
    .await;

    let mut headers = HeaderMap::new();
    headers.insert("hx-refresh", "true".parse().unwrap());

    return (headers).into_response();
}

pub async fn play(
    State(client): State<PgPool>,
    LoggedUser(_user_id): LoggedUser,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {

    let _ = sqlx::query!(
        r#"update website set is_paused=false
            where id = $1  "#,
        id
    )
    .execute(&client)
    .await;

    let mut headers = HeaderMap::new();
    headers.insert("hx-refresh", "true".parse().unwrap());

    return (headers).into_response();
}


pub fn router() -> Router<AppState> {
    Router::new()
        .route("/create", get(get_website_create))
        .route("/", post(create))
        .route(
            "/:id",
            get(get_website).delete(delete).put(update),
        )
        .route("/:id/pause", post(pause))
        .route("/:id/play", post(play))
}
