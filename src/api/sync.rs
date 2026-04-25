use axum::{
    Extension, Json, Router,
    extract::{FromRequest, Query},
    middleware,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use tracing::{info, instrument};

use crate::{
    api::{
        auth,
        response::ApiResult,
        state::{AppState, AuthState, AuthStateExtractor},
    },
    db::{
        self,
        schema::{Book, BookConfig, BookNote},
    },
    error::Error,
    utils::serdefmt::empty_str_as_none,
};

pub fn router() -> Router {
    Router::new()
        .route("/sync", get(pull_sync))
        .route("/sync", post(push_sync))
        .route_layer(middleware::from_fn(auth::auth_middleware))
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct SyncData {
    #[serde(default)]
    books: Vec<Book>,
    #[serde(default)]
    configs: Vec<BookConfig>,
    #[serde(default)]
    notes: Vec<BookNote>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum SyncType {
    Books,
    Configs,
    Notes,
}

#[derive(Debug, Deserialize)]
struct PullSyncQueryParam {
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub since: chrono::DateTime<chrono::Utc>,

    #[serde(rename = "sync", default)]
    pub sync_type: Option<SyncType>,

    #[serde(rename = "book", deserialize_with = "empty_str_as_none", default)]
    pub book_hash: Option<String>,

    #[serde(deserialize_with = "empty_str_as_none", default)]
    pub meta_hash: Option<String>,
}

#[derive(Debug, FromRequest)]
#[from_request(rejection(Error))]
struct PullSyncExtractor {
    state: Extension<AppState>,
    auth: Extension<AuthState>,
    query: Query<PullSyncQueryParam>,
}

#[instrument(skip(state, auth))]
async fn pull_sync(
    PullSyncExtractor {
        state,
        auth,
        query: Query(query),
    }: PullSyncExtractor,
) -> ApiResult<Json<SyncData>> {
    let mut data = SyncData::default();

    let (want_books, want_configs, want_notes) = match query.sync_type {
        Some(SyncType::Books) => (true, false, false),
        Some(SyncType::Configs) => (false, true, false),
        Some(SyncType::Notes) => (false, false, true),
        None => (true, true, true),
    };

    if want_books {
        info!(%query.since, "pull books");
        data.books = db::book::get_books(
            &state.pool,
            &auth.user.id,
            query.since,
            query.book_hash.clone(),
            query.meta_hash.clone(),
        )
        .await?;
    }
    if want_configs {
        info!(%query.since, "pull books");
        data.configs = db::config::get_book_configs(
            &state.pool,
            &auth.user.id,
            query.since,
            query.book_hash.clone(),
            query.meta_hash.clone(),
        )
        .await?;
    }
    if want_notes {
        info!(%query.since, "pull books");
        data.notes = db::note::get_book_notes(
            &state.pool,
            &auth.user.id,
            query.since,
            query.book_hash,
            query.meta_hash,
        )
        .await?;
    }

    Ok(Json(data))
}

#[derive(Debug, FromRequest)]
#[from_request(rejection(Error))]
struct PushSyncExtractor {
    body: Json<SyncData>,
}

#[instrument(skip(state, auth, body))]
async fn push_sync(
    AuthStateExtractor { state, auth }: AuthStateExtractor,
    PushSyncExtractor { body: Json(body) }: PushSyncExtractor,
) -> ApiResult<Json<SyncData>> {
    let mut data = SyncData::default();
    if !body.books.is_empty() {
        data.books = db::book::upsert_books(&state.pool, &auth.user.id, body.books).await?;
    }
    if !body.configs.is_empty() {
        data.configs =
            db::config::upsert_book_configs(&state.pool, &auth.user.id, body.configs).await?;
    }
    if !body.notes.is_empty() {
        data.notes = db::note::upsert_book_notes(&state.pool, &auth.user.id, body.notes).await?;
    }
    Ok(Json(data))
}
