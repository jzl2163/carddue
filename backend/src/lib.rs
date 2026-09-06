pub mod auth;
pub mod calendar;
pub mod cards;
pub mod config;
pub mod crypto;
pub mod dates;
pub mod error;
pub mod model;
pub mod network;
pub mod notifications;
pub mod planner;
pub mod providers;
pub mod spec;
pub mod template;
pub mod web;
pub mod worker;

use std::sync::Arc;
pub struct StateInner {
    pub db: sqlx::PgPool,
    pub config: config::Config,
    pub passwords: Arc<tokio::sync::Semaphore>,
}
pub type AppState = Arc<StateInner>;
