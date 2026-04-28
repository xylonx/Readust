use std::collections::{HashMap, HashSet};

use axum::{
    Json, Router,
    extract::{FromRequest, FromRequestParts, Query},
    response::IntoResponse,
    routing::{delete, get, post},
};
use serde::{Deserialize, Serialize};
use tracing::{debug, error, instrument};
use validator::Validate;

use crate::{
    api::{
        response::ApiResult,
        state::{AppState, AuthStateExtractor},
        validator::ValidatedJson,
    },
    db::{self},
    error::Error,
    utils::{
        safepath::SafePathBuf,
        serdefmt::{empty_str_as_none, ok_or_default},
    },
};

pub fn router() -> Router {
    Router::new().nest(
        "/storage",
        Router::new()
            .route("/list", get(list))
            .route("/stats", get(stats))
            .route("/purge", delete(purge))
            .route("/upload", post(upload))
            .route("/download", get(download_single))
            .route("/download", post(download_multiple))
            .route("/delete", delete(delete_file)),
    )
}

fn default_page() -> u64 {
    1
}

pub fn der_page_size<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Deserialize;
    let input_size = Option::<u64>::deserialize(deserializer)?.unwrap_or(50);
    Ok(input_size.clamp(1, 100))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListFilesQuery {
    #[serde(default = "default_page")]
    page: u64,
    #[serde(deserialize_with = "der_page_size")]
    page_size: u64,
    #[serde(default)]
    sort_by: db::schema::SortByKind,
    #[serde(deserialize_with = "ok_or_default", default)]
    sort_order: db::schema::SortOrderKind,
    #[serde(deserialize_with = "empty_str_as_none", default)]
    book_hash: Option<String>,
    #[serde(deserialize_with = "empty_str_as_none", default, rename = "search")]
    file_key: Option<String>,
}

#[derive(Debug, FromRequestParts)]
#[from_request(rejection(Error))]
struct ListFilesExtractor {
    query: Query<ListFilesQuery>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ListFilesResponse {
    files: Vec<db::schema::File>,
    total: u64,
    page: u64,
    page_size: u64,
    total_pages: u64,
}

async fn list(
    AuthStateExtractor { auth, state }: AuthStateExtractor,
    ListFilesExtractor { query }: ListFilesExtractor,
) -> ApiResult<Json<ListFilesResponse>> {
    let (total_cnt, search_files) = futures_util::future::try_join(
        db::file::agg_files_by_book_hash_file_key(
            &state.pool,
            &auth.user.id,
            &query.book_hash,
            &query.file_key,
        ),
        db::file::get_files_by_page(
            &state.pool,
            &auth.user.id,
            &query.book_hash,
            &query.file_key,
            &query.sort_by,
            &query.sort_order,
            query.page as i64,
            query.page_size as i64,
        ),
    )
    .await?;

    // From Readest:
    //
    // Fetch all files with the same book_hashes to ensure complete book groups
    // IMPORTANT: We don't apply the search filter here. This ensures that ALL files
    // for matched books are included (e.g., cover.png files), even if they don't
    // match the search term. This is crucial for proper book grouping and selection.
    let hashes = search_files
        .iter()
        .filter_map(|f| f.book_hash.clone())
        .collect::<HashSet<_>>() // Collect to set to dedup
        .into_iter()
        .collect::<Vec<_>>();

    let files = match db::file::get_files_by_book_hashes(&state.pool, &auth.user.id, &hashes).await
    {
        Ok(hash_match_files) => search_files
            .into_iter()
            .chain(hash_match_files)
            .map(|file| (file.file_key.to_string(), file))
            .collect::<HashMap<_, _>>()
            .into_values()
            .collect::<Vec<_>>(),
        Err(e) => {
            error!(%e,"failed to list files with corresponding book hash. Ignore it");
            search_files
        }
    };

    Ok(Json(ListFilesResponse {
        files,
        total: total_cnt as u64,
        page: query.page,
        page_size: query.page_size,
        total_pages: (total_cnt as u64).div_ceil(query.page_size),
    }))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct StorageStatResponse {
    total_files: i64,
    total_size: i64,
    usage: i64,
    quota: i64,
    usage_percentage: f64,
    by_book_hash: Vec<db::schema::BookHashStat>,
}

async fn stats(
    AuthStateExtractor { auth, state }: AuthStateExtractor,
) -> ApiResult<Json<StorageStatResponse>> {
    let (total_size, cnt) = db::file::sum_file_count_size(&state.pool, &auth.user.id).await?;

    // TODO(xylonx): QUOTA is not supported now. PLAN in the future.

    let agg_stat = db::file::agg_file_count_size_by_book_hash(&state.pool, &auth.user.id).await?;

    Ok(Json(StorageStatResponse {
        total_files: cnt,
        total_size,
        usage: total_size,
        quota: i64::MAX,
        usage_percentage: 0.0,
        by_book_hash: agg_stat,
    }))
}

#[derive(Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
struct PurgeBody {
    #[validate(length(
        min = 1,
        max = 100,
        message = "Could only purge files between 1 and 100"
    ))]
    file_keys: Vec<SafePathBuf>,
}

#[derive(Debug, FromRequest)]
#[from_request(rejection(Error))]
struct PurgeExtractor {
    #[from_request(via(ValidatedJson))]
    body: PurgeBody,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PurgeFailed {
    file_key: String,
    error: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PurgeResponse {
    success: Vec<String>,
    failed: Vec<PurgeFailed>,
    deleted_count: i64,
    failed_count: i64,
}

async fn purge(
    AuthStateExtractor { auth, state }: AuthStateExtractor,
    PurgeExtractor { body }: PurgeExtractor,
) -> ApiResult<Json<PurgeResponse>> {
    let file_str_keys = body.file_keys.into_iter().map(|e| e.to_string()).collect();
    let files = db::file::get_file_by_file_keys(&state.pool, &auth.user.id, &file_str_keys).await?;

    if files.is_empty() {
        return Err(Error::FileNotFound);
    }
    if files.len() != file_str_keys.len() {
        return Err(Error::Unauthorized(
            "Unauthorized access to one or more files".to_string(),
        ));
    }

    let results = futures_util::future::join_all(files.into_iter().map(|file| {
        let state1 = state.clone();
        async move {
            match purge_file(state1, &file.id, &file.file_key).await {
                Ok(_) => Ok(file.file_key),
                Err(e) => Err(PurgeFailed {
                    file_key: file.file_key,
                    error: e.to_string(),
                }),
            }
        }
    }))
    .await;

    let (success, failed) =
        results
            .into_iter()
            .fold((vec![], vec![]), |(mut succ, mut failed), item| {
                match item {
                    Ok(file_key) => succ.push(file_key),
                    Err(e) => failed.push(e),
                };
                (succ, failed)
            });

    Ok(Json(PurgeResponse {
        // It is SAFE to cast len to i64 directly since the max length of files is 100
        deleted_count: success.len() as i64,
        failed_count: failed.len() as i64,
        success,
        failed,
    }))
}

#[instrument(skip(state))]
async fn purge_file(state: AppState, file_id: &uuid::Uuid, file_key: &str) -> ApiResult<()> {
    state.s3_client.delete_object(file_key).await?;
    let file = db::file::delete_file_by_id(&state.pool, file_id).await?;
    debug!(?file, "delete file");
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UploadBody {
    #[serde(deserialize_with = "empty_str_as_none", default)]
    book_hash: Option<String>,
    file_name: SafePathBuf,
    file_size: i64,
    #[serde(default)]
    temp: bool,
}

#[derive(Debug, FromRequest)]
#[from_request(rejection(Error))]
struct UploadExtractor {
    #[from_request(via(Json))]
    body: UploadBody,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct UploadResponse {
    upload_url: String,
    file_key: String,
    usage: u64,
    quota: i64,
}

async fn upload(
    AuthStateExtractor { auth, state }: AuthStateExtractor,
    UploadExtractor { body }: UploadExtractor,
) -> ApiResult<Json<UploadResponse>> {
    if body.temp {
        return Err(Error::TempUploadUnsupported);
    }

    // TODO(xylonx): Currently, we don't have quota restriction for the user. Plan in the future

    let file_key = format!("{}/{}", auth.user.id, body.file_name);

    let file_size = match db::file::get_file_by_key(&state.pool, &auth.user.id, &file_key).await? {
        Some(file) => file.file_size, // File already exists. reuse it
        None => {
            // File does not exist. Create a new record on database
            db::file::insert_file(
                &state.pool,
                &auth.user.id,
                body.book_hash,
                &file_key,
                body.file_size,
            )
            .await?
            .file_size
        }
    };

    let presigned_url = state
        .s3_client
        .presign_upload_url(&file_key, file_size)
        .await?;

    Ok(Json(UploadResponse {
        upload_url: presigned_url,
        file_key,
        usage: 0,
        quota: i64::MAX,
    }))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DownloadFileQuery {
    file_key: SafePathBuf,
}

#[derive(Debug, FromRequestParts)]
#[from_request(rejection(Error))]
struct DownloadSingleExtractor {
    #[from_request(via(Query))]
    query: DownloadFileQuery,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DownloadResponse {
    download_url: String,
}

async fn download_single(
    AuthStateExtractor { auth, state }: AuthStateExtractor,
    DownloadSingleExtractor { query }: DownloadSingleExtractor,
) -> ApiResult<Json<DownloadResponse>> {
    let file_key = query.file_key.to_string();
    let map = generate_download_url_map(&state, &auth.user.id, vec![file_key.to_string()]).await?;
    match map.get(&file_key) {
        Some(url) => Ok(Json(DownloadResponse {
            download_url: url.clone(),
        })),
        None => Err(Error::FileNotFound),
    }
}

#[derive(Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
struct DownloadMultipleBody {
    #[validate(length(
        min = 1,
        max = 100,
        message = "Could only purge files between 1 and 100"
    ))]
    file_keys: Vec<SafePathBuf>,
}

#[derive(Debug, FromRequest)]
#[from_request(rejection(Error))]
struct DownloadMultipleExtractor {
    #[from_request(via(ValidatedJson))]
    body: DownloadMultipleBody,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DownloadMultipleResponse {
    // download url map: file_key -> download_url
    download_urls: HashMap<String, String>,
}

async fn download_multiple(
    AuthStateExtractor { auth, state }: AuthStateExtractor,
    DownloadMultipleExtractor { body }: DownloadMultipleExtractor,
) -> ApiResult<Json<DownloadMultipleResponse>> {
    let file_keys = body.file_keys.iter().map(ToString::to_string).collect();
    let map = generate_download_url_map(&state, &auth.user.id, file_keys).await?;
    Ok(Json(DownloadMultipleResponse { download_urls: map }))
}

async fn generate_download_url_map(
    state: &AppState,
    user_id: &uuid::Uuid,
    file_keys: Vec<String>,
) -> ApiResult<HashMap<String, String>> {
    let files = db::file::get_file_by_file_keys(&state.pool, user_id, &file_keys).await?;

    let db_file_map = files
        .into_iter()
        .map(|f| (f.file_key.clone(), f))
        .collect::<HashMap<_, _>>();

    let missing_file_key_map = file_keys
        .into_iter()
        .filter(|file_key| !db_file_map.contains_key(file_key) && file_key.contains("Readest/Book"))
        .filter_map(|file_key| {
            let file_key1 = file_key.clone();
            let mut splits = file_key1.split("/");
            let book_hash = splits.nth(3); // The 4-th element
            let filename = splits.next(); // The 5-th element

            // It contains exactly 5 parts. Valid format
            if let Some(book_hash) = book_hash
                && let Some(_) = filename
                && splits.next().is_none()
            {
                let file_extension = std::path::Path::new(&file_key)
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(ToString::to_string)
                    .unwrap_or_default();
                Some((book_hash.to_string(), (file_key, file_extension)))
            } else {
                None
            }
        })
        .collect::<HashMap<_, _>>();

    let hashes = missing_file_key_map
        .keys()
        .map(ToOwned::to_owned)
        .collect::<HashSet<_>>() // Collect to set to dedup
        .into_iter()
        .collect::<Vec<_>>();

    let fallback_file_map = db::file::get_files_by_book_hashes(&state.pool, user_id, &hashes)
        .await
        .into_iter()
        .flat_map(|files| {
            files.into_iter().filter_map(|file| {
                if let Some(hash) = &file.book_hash
                    && let Some((original_file_key, ext)) = missing_file_key_map.get(hash)
                    && file.file_key.ends_with(&format!(".{}", ext))
                {
                    Some(original_file_key.to_string())
                } else {
                    None
                }
            })
        });

    let result =
        futures_util::future::join_all(db_file_map.into_keys().chain(fallback_file_map).map(
            |file_key| {
                let state1 = state.clone();
                async move {
                    match state1.s3_client.presign_download_url(&file_key).await {
                        Ok(url) => Some((file_key, url)),
                        Err(e) => {
                            error!(file_key, %e, "Error creating signed URL");
                            None
                        }
                    }
                }
            },
        ))
        .await;

    Ok(result.into_iter().flatten().collect())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeleteFileQuery {
    file_key: SafePathBuf,
}

#[derive(Debug, FromRequestParts)]
#[from_request(rejection(Error))]
struct DeleteFileExtractor {
    #[from_request(via(Query))]
    query: DeleteFileQuery,
}

async fn delete_file(
    AuthStateExtractor { auth, state }: AuthStateExtractor,
    DeleteFileExtractor { query }: DeleteFileExtractor,
) -> ApiResult<impl IntoResponse> {
    let file_key = query.file_key.to_string();
    match db::file::get_file_by_key(&state.pool, &auth.user.id, &file_key).await? {
        Some(_) => {
            state.s3_client.delete_object(&file_key).await?;
            Ok(Json(serde_json::json!({
                "message": "File deleted successfully"
            })))
        }
        None => Err(Error::FileNotFound),
    }
}
