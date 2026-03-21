use std::sync::Arc;

use sqlx::PgPool;
use webauthn_rs::prelude::*;

use super::config::AppConfig;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub config: AppConfig,
    pub webauthn: Arc<Webauthn>,
}
