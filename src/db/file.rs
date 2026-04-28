use tracing::instrument;

use crate::db::schema;

#[instrument(skip(pool))]
pub async fn get_file_by_key(
    pool: &sqlx::PgPool,
    user_id: &uuid::Uuid,
    file_key: &str,
) -> Result<Option<schema::File>, sqlx::Error> {
    sqlx::query_as!(
        schema::File,
        r#"SELECT * FROM files WHERE user_id = $1 AND file_key = $2"#,
        user_id,
        file_key
    )
    .fetch_optional(pool)
    .await
}

#[instrument(skip(pool, file_keys))]
pub async fn get_file_by_file_keys(
    pool: &sqlx::PgPool,
    user_id: &uuid::Uuid,
    file_keys: &Vec<String>,
) -> Result<Vec<schema::File>, sqlx::Error> {
    sqlx::query_as!(
        schema::File,
        r#"
        SELECT *
        FROM files
        WHERE user_id = $1
          AND file_key IN ( SELECT UNNEST( $2::text[] ) )
          AND deleted_at IS NULL
        "#,
        user_id,
        file_keys,
    )
    .fetch_all(pool)
    .await
}

#[instrument(skip(pool, book_hashes))]
pub async fn get_files_by_book_hashes(
    pool: &sqlx::PgPool,
    user_id: &uuid::Uuid,
    book_hashes: &[String],
) -> Result<Vec<schema::File>, sqlx::Error> {
    if book_hashes.is_empty() {
        return Ok(vec![]);
    }

    sqlx::query_as!(
        schema::File,
        r#"
        SELECT *
        FROM files
        WHERE user_id = $1
          AND book_hash IN ( SELECT UNNEST( $2::text[] ) )
          AND deleted_at IS NULL
        "#,
        user_id,
        book_hashes,
    )
    .fetch_all(pool)
    .await
}

#[instrument(skip(pool))]
pub async fn sum_file_count_size(
    pool: &sqlx::PgPool,
    user_id: &uuid::Uuid,
) -> Result<(i64, i64), sqlx::Error> {
    let row = sqlx::query!(
        r#"
        SELECT
            COALESCE(SUM(file_size), 0)::BIGINT AS "total_size!",
            COUNT(*)::BIGINT AS "file_count!"
        FROM files
        WHERE user_id = $1
          AND deleted_at IS NULL
        "#,
        user_id
    )
    .fetch_one(pool)
    .await?;
    Ok((row.total_size, row.file_count))
}

#[instrument(skip(pool))]
pub async fn agg_file_count_size_by_book_hash(
    pool: &sqlx::PgPool,
    user_id: &uuid::Uuid,
) -> Result<Vec<schema::BookHashStat>, sqlx::Error> {
    sqlx::query_as!(
        schema::BookHashStat,
        r#"
        SELECT
          book_hash,
          COALESCE(SUM(file_size), 0)::BIGINT AS "total_size!",
          COUNT(*)::BIGINT AS "file_count!"
        FROM files
        WHERE user_id = $1
          AND deleted_at IS NULL
        GROUP BY book_hash
        ORDER BY "total_size!" DESC
        "#,
        user_id
    )
    .fetch_all(pool)
    .await
}

#[instrument(skip(pool))]
pub async fn get_files_by_page(
    pool: &sqlx::PgPool,
    user_id: &uuid::Uuid,
    book_hash: &Option<String>,
    file_key: &Option<String>,
    sort_by: &schema::SortByKind,
    sort_order: &schema::SortOrderKind,
    page: i64,
    page_size: i64,
) -> Result<Vec<schema::File>, sqlx::Error> {
    let offset = (page - 1) * page_size;
    sqlx::query_as!(
        schema::File,
        r#"
        SELECT *
        FROM public.files
        WHERE
          user_id = $1
          AND deleted_at IS NULL
          AND ($2::text IS NULL OR book_hash = $2)
          AND ($3::text IS NULL OR file_key ILIKE '%' || $3 || '%')
        ORDER BY
          CASE WHEN $6 = 'created_at' AND $7 = 'asc'  THEN created_at END ASC,
          CASE WHEN $6 = 'created_at' AND $7 = 'desc' THEN created_at END DESC,
 
          CASE WHEN $6 = 'updated_at' AND $7 = 'asc'  THEN updated_at END ASC,
          CASE WHEN $6 = 'updated_at' AND $7 = 'desc' THEN updated_at END DESC,

          CASE WHEN $6 = 'file_key' AND $7 = 'asc'  THEN file_key END ASC,
          CASE WHEN $6 = 'file_key' AND $7 = 'desc' THEN file_key END DESC,

          CASE WHEN $6 = 'file_size' AND $7 = 'asc'  THEN file_size END ASC,
          CASE WHEN $6 = 'file_size' AND $7 = 'desc' THEN file_size END DESC,

          id ASC
        LIMIT $4
        OFFSET $5
        "#,
        user_id,
        book_hash.as_deref(),
        file_key.as_deref(),
        page_size,
        offset,
        sort_by.as_sql_column(),
        sort_order.as_sql_direction(),
    )
    .fetch_all(pool)
    .await
}

#[instrument(skip(pool))]
pub async fn agg_files_by_book_hash_file_key(
    pool: &sqlx::PgPool,
    user_id: &uuid::Uuid,
    book_hash: &Option<String>,
    file_key: &Option<String>,
) -> Result<i64, sqlx::Error> {
    sqlx::query!(
        r#"
        SELECT COUNT(*) AS "total_count!"
        FROM files
        WHERE
            user_id = $1
            AND deleted_at IS NULL
            AND ($2::text IS NULL OR book_hash = $2)
            AND ($3::text IS NULL OR file_key ILIKE '%' || $3 || '%')"#,
        user_id,
        book_hash.as_deref(),
        file_key.as_deref(),
    )
    .fetch_one(pool)
    .await
    .map(|record| record.total_count)
}

#[instrument(skip(pool))]
pub async fn insert_file(
    pool: &sqlx::PgPool,
    user_id: &uuid::Uuid,
    book_hash: Option<String>,
    file_key: &str,
    file_size: i64,
) -> Result<schema::File, sqlx::Error> {
    sqlx::query_as!(
        schema::File,
        r#"
        INSERT INTO files ( user_id, book_hash, file_key, file_size )
        VALUES ( $1, $2, $3, $4 )
        RETURNING *
        "#,
        user_id,
        book_hash,
        file_key,
        file_size
    )
    .fetch_one(pool)
    .await
}

#[instrument(skip(pool))]
pub async fn delete_file_by_id(
    pool: &sqlx::PgPool,
    id: &uuid::Uuid,
) -> Result<schema::File, sqlx::Error> {
    sqlx::query_as!(
        schema::File,
        r#"
        UPDATE files SET deleted_at = NOW() WHERE id = $1
        RETURNING *
        "#,
        id
    )
    .fetch_one(pool)
    .await
}
