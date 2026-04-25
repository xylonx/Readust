use crate::db::schema;

pub async fn get_book_configs(
    pool: &sqlx::PgPool,
    user_id: &uuid::Uuid,
    since: chrono::DateTime<chrono::Utc>,
    book_hash: Option<String>,
    meta_hash: Option<String>,
) -> Result<Vec<schema::BookConfig>, sqlx::Error> {
    sqlx::query_as!(
        schema::BookConfig,
        r#"
        SELECT *
        FROM book_configs
        WHERE user_id = $1
          AND updated_at > $2
          AND deleted_at IS NULL
          AND ($3::text IS NULL OR book_hash = $3)
          AND ($4::text IS NULL OR meta_hash = $4)
        "#,
        user_id,
        since,
        book_hash,
        meta_hash,
    )
    .fetch_all(pool)
    .await
}

pub async fn upsert_book_configs(
    pool: &sqlx::PgPool,
    user_id: &uuid::Uuid,
    configs: Vec<schema::BookConfig>,
) -> Result<Vec<schema::BookConfig>, sqlx::Error> {
    let mut data = Vec::new();

    for config in configs {
        let record = sqlx::query_as!(
            schema::BookConfig,
            r#"
            INSERT INTO public.book_configs ( user_id, book_hash, meta_hash, location, xpointer, progress, rsvp_position, search_config, view_settings, deleted_at )
            VALUES ( $1, $2, $3, $4, $5, $6, $7, $8, $9, $10 )
            ON CONFLICT (user_id, book_hash)
            DO UPDATE SET
              meta_hash     = EXCLUDED.meta_hash,
              location      = EXCLUDED.location,
              xpointer      = EXCLUDED.xpointer,
              progress      = EXCLUDED.progress,
              rsvp_position = EXCLUDED.rsvp_position,
              search_config = EXCLUDED.search_config,
              view_settings = EXCLUDED.view_settings,
              updated_at    = NOW(),
              deleted_at    = EXCLUDED.deleted_at
            RETURNING *
            "#,
            user_id,
            config.book_hash,
            config.meta_hash,
            config.location,
            config.xpointer,
            config.progress,
            config.rsvp_position,
            config.search_config,
            config.view_settings,
            config.deleted_at
        )
        .fetch_one(pool)
        .await?;

        data.push(record);
    }

    Ok(data)
}
