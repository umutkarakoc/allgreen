extern crate core;

use crate::appconfig::ENV;
use async_session::CookieStore;
use axum::{extract::FromRef, http::StatusCode, response::IntoResponse, routing::*, Router};

use sqlx::{postgres::PgPoolOptions, PgPool};
use std::io;
use tower_http::services::ServeDir;
mod app;
mod appconfig;
mod checker;
mod logged_user;
mod models;

#[derive(Clone)]
pub struct AppState {
    db: PgPool,
    session_store: CookieStore,
}
impl FromRef<AppState> for PgPool {
    fn from_ref(app_state: &AppState) -> PgPool {
        app_state.db.clone()
    }
}
impl FromRef<AppState> for CookieStore {
    fn from_ref(app_state: &AppState) -> CookieStore {
        app_state.session_store.clone()
    }
}

#[tokio::main]
async fn main() {
    let db = PgPoolOptions::new()
        .max_connections(20)
        .connect(&ENV.database_url)
        .await
        .expect("can connect to database");

    let checker_db = db.clone();

    tokio::spawn(async move {
        checker::run(checker_db).await;
    });

    let _secret = &ENV.secret.clone().into_bytes()[..];
    let session_store = CookieStore::new();

    let app_state = AppState { db, session_store };

    let serve_dir = get_service(ServeDir::new(ENV.assets.clone())).handle_error(handle_error);

    let app = Router::new()
        .nest("/", app::router())
        .with_state(app_state)
        .fallback_service(serve_dir);

    axum::Server::bind(&ENV.addr.parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn handle_error(_err: io::Error) -> impl IntoResponse {
    (StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong...")
}
