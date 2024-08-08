use axum::Router;
use crate::AppState;

mod logo;
mod auth;
mod home;
mod layout;
mod website;
mod mail;

pub fn router() -> Router<AppState> {
    Router::new()
        .nest("/", home::router())
        .nest("/auth",  auth::router())
        .nest("/website", website::router())
}
