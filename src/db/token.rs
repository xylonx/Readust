use tracing::instrument;
use uuid::Uuid;

use crate::db::schema;

#[instrument(skip(pool))]
pub async fn create_token(
    pool: &sqlx::PgPool,
    token_id: Uuid,
    refresh_token: Uuid,
    user_id: Uuid,
    expires_at: chrono::DateTime<chrono::Utc>,
) -> Result<schema::Token, sqlx::Error> {
    sqlx::query_as!(
        schema::Token,
        r#"
        INSERT INTO tokens ( id, user_id, refresh_token, expires_at )
        VALUES ( $1, $2, $3, $4 )
        RETURNING *
        "#,
        token_id,
        user_id,
        refresh_token,
        expires_at,
    )
    .fetch_one(pool)
    .await
}

#[instrument(skip(pool))]
pub async fn get_token_by_id(
    pool: &sqlx::PgPool,
    token_id: &Uuid,
) -> Result<schema::Token, sqlx::Error> {
    sqlx::query_as!(
        schema::Token,
        r#"SELECT * FROM tokens WHERE id = $1 AND deleted_at IS NULL"#,
        token_id
    )
    .fetch_one(pool)
    .await
}

#[instrument(skip(pool))]
pub async fn delete_token(pool: &sqlx::PgPool, token_id: &Uuid) -> Result<(), sqlx::Error> {
    sqlx::query_as!(
        schema::Token,
        r#"UPDATE tokens SET deleted_at = NOW() WHERE id = $1"#,
        token_id
    )
    .execute(pool)
    .await
    .map(|_| ())
}

#[instrument(skip(pool))]
pub async fn delete_token_by_user_id(
    pool: &sqlx::PgPool,
    user_id: &Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query_as!(
        schema::Token,
        r#"UPDATE tokens SET deleted_at = NOW() WHERE user_id = $1"#,
        user_id
    )
    .execute(pool)
    .await
    .map(|_| ())
}

pub async fn delete_token_by_refresh_token(
    pool: &sqlx::PgPool,
    refresh_token: &Uuid,
) -> Result<schema::Token, sqlx::Error> {
    sqlx::query_as!(
        schema::Token,
        r#"UPDATE tokens SET deleted_at = NOW() WHERE refresh_token = $1 RETURNING *"#,
        refresh_token
    )
    .fetch_one(pool)
    .await
}
