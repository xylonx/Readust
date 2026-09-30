use std::collections::HashSet;

use axum::{
    Extension, Json, Router,
    extract::{FromRequest, Query},
    response::{IntoResponse, Response},
    routing::get,
};
use serde::{Deserialize, Serialize};
use validator::{Validate, ValidateArgs};

use crate::{
    api::{
        response::ApiResult,
        state::{AppState, AuthState},
    },
    db::{
        replicas::{get_replicas, merge_replicas},
        schema::{ReplicaKind, ReplicaRow},
    },
    error::Error,
};

pub fn router() -> Router {
    Router::new().route("/sync/replicas", get(pull).post(batch_ops))
}

#[derive(Debug, Serialize)]
struct ReplicasData {
    rows: Vec<ReplicaRow>,
}

#[derive(Debug, Deserialize)]
struct PullReplicasQuery {
    kind: ReplicaKind,
    since: Option<String>,
}

#[derive(Debug, FromRequest)]
#[from_request(rejection(Error))]
struct PullReplicasExtractor {
    #[from_request(via(Extension))]
    state: AppState,
    #[from_request(via(Extension))]
    auth: AuthState,
    #[from_request(via(Query))]
    query: PullReplicasQuery,
}

#[derive(Debug, Serialize, Deserialize)]
struct BatchPullEntry {
    kind: ReplicaKind,
    since: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
struct BatchPullBody {
    #[validate(custom(function = "validate_unique_kind"))]
    cursors: Vec<BatchPullEntry>,
}

#[derive(Debug, Deserialize, Validate)]
#[validate(context = "AuthState")]
struct BatchPushBody {
    #[validate(custom(function = "validate_batch_push_body", use_context))]
    rows: Vec<ReplicaRow>,
}

#[derive(Debug, Serialize)]
pub struct BatchPullResponse {
    results: Vec<BatchPullResponseEntry>,
}

#[derive(Debug, Serialize)]
pub struct BatchPullResponseEntry {
    kind: ReplicaKind,
    rows: Vec<ReplicaRow>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum BatchBody {
    Pull(BatchPullBody),
    Push(BatchPushBody),
}

#[derive(Debug, FromRequest)]
#[from_request(rejection(Error))]
struct BatchReplicasExtractor {
    #[from_request(via(Extension))]
    state: AppState,
    #[from_request(via(Extension))]
    auth: AuthState,
    #[from_request(via(Json))]
    body: BatchBody,
}

async fn batch_ops(
    BatchReplicasExtractor { state, auth, body }: BatchReplicasExtractor,
) -> ApiResult<Response> {
    match body {
        BatchBody::Pull(body) => {
            body.validate()?;
            Ok(batch_pull(state, auth, body.cursors).await?.into_response())
        }
        BatchBody::Push(body) => {
            body.validate_with_args(&auth)?;
            Ok(batch_push(state, body.rows).await?.into_response())
        }
    }
}

async fn batch_pull(
    state: AppState,
    auth: AuthState,
    cursors: Vec<BatchPullEntry>,
) -> ApiResult<Json<BatchPullResponse>> {
    let results = futures_util::future::try_join_all(cursors.iter().map(|entry| {
        let pool = state.pool.clone();
        async move {
            Result::<_, sqlx::Error>::Ok(BatchPullResponseEntry {
                kind: entry.kind.clone(),
                rows: get_replicas(&pool, &auth.user.id, &entry.kind, entry.since.as_ref()).await?,
            })
        }
    }))
    .await?;
    Ok(Json(BatchPullResponse { results }))
}

async fn batch_push(state: AppState, rows: Vec<ReplicaRow>) -> ApiResult<Json<ReplicasData>> {
    let mut merged = vec![];

    for row in rows {
        let record = merge_replicas(&state.pool, &row).await?;
        merged.push(record);
    }

    Ok(Json(ReplicasData { rows: merged }))
}

async fn pull(
    PullReplicasExtractor { state, auth, query }: PullReplicasExtractor,
) -> ApiResult<Json<ReplicasData>> {
    let rows = get_replicas(
        &state.pool,
        &auth.user.id,
        &query.kind,
        query.since.as_ref(),
    )
    .await?;
    Ok(Json(ReplicasData { rows }))
}

fn validate_unique_kind(cursors: &[BatchPullEntry]) -> Result<(), validator::ValidationError> {
    let mut seen = HashSet::new();
    let dup = cursors
        .iter()
        .find(|e| !seen.insert(e.kind.clone()))
        .map(|e| e.kind.clone());

    match dup {
        Some(k) => Err(validator::ValidationError::new("unique_pull_kind")
            .with_message(format!("duplicated pull kind {}", k.as_str()).into())),
        None => Ok(()),
    }
}

fn validate_batch_push_body(
    rows: &[ReplicaRow],
    context: &AuthState,
) -> Result<(), validator::ValidationError> {
    for row in rows {
        if row.user_id != context.user.id {
            return Err(validator::ValidationError::new("batch_push_body")
                .with_message("contains row doesn't belong to current user".into()));
        }
    }
    Ok(())
}
