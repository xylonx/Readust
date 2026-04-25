use std::{ops::Deref, sync::Arc};

use axum::{Extension, extract::FromRequestParts};

use crate::{
    db::schema,
    error::Error,
    utils::{jwt::JwtClient, s3::S3Client},
};

#[derive(Debug, Clone)]
pub struct AppState(Arc<AppStateInner>);

impl Deref for AppState {
    type Target = AppStateInner;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug)]
pub struct AppStateInner {
    pub pool: sqlx::PgPool,

    pub jwt_client: JwtClient,
    pub anon_token: String,
    pub disable_signup: bool,

    pub s3_client: S3Client,
}

impl AppState {
    pub fn new(
        pool: sqlx::PgPool,
        anon_token: String,
        jwt_client: JwtClient,
        disable_signup: bool,
        s3_client: S3Client,
    ) -> Self {
        Self(Arc::new(AppStateInner {
            pool,
            anon_token,
            jwt_client,
            disable_signup,
            s3_client,
        }))
    }
}

#[derive(Debug, Clone)]
pub struct AuthState {
    pub user: schema::User,
    pub token_id: uuid::Uuid,
}

#[derive(Debug, FromRequestParts)]
#[from_request(rejection(Error))]
pub struct StateExtractor {
    #[from_request(via(Extension))]
    pub state: AppState,
}

#[derive(Debug, FromRequestParts)]
#[from_request(rejection(Error))]
pub struct AuthStateExtractor {
    #[from_request(via(Extension))]
    pub state: AppState,
    #[from_request(via(Extension))]
    pub auth: AuthState,
}
