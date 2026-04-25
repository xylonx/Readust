use uuid::Uuid;

use crate::db::schema;

pub async fn get_user_by_id(
    pool: &sqlx::PgPool,
    user_id: &Uuid,
) -> Result<Option<schema::User>, sqlx::Error> {
    sqlx::query_as!(
        schema::User,
        r#"SELECT * FROM users WHERE id = $1 AND deleted_at IS NULL"#,
        user_id,
    )
    .fetch_optional(pool)
    .await
}

pub async fn get_user_by_email(
    pool: &sqlx::PgPool,
    email: &str,
) -> Result<Option<schema::User>, sqlx::Error> {
    sqlx::query_as!(
        schema::User,
        r#"SELECT * FROM users WHERE email = $1 AND deleted_at IS NULL"#,
        email
    )
    .fetch_optional(pool)
    .await
}

pub async fn insert_user(
    pool: &sqlx::PgPool,
    email: &str,
    encrypted_password: &str,
) -> Result<schema::User, sqlx::Error> {
    sqlx::query_as!(
        schema::User,
        r#"
        INSERT INTO users ( email, encrypted_password )
        VALUES ( $1, $2 )
        RETURNING *
        "#,
        email,
        encrypted_password
    )
    .fetch_one(pool)
    .await
}
