use crate::db::schema;

pub async fn get_book_notes(
    pool: &sqlx::PgPool,
    user_id: &uuid::Uuid,
    since: chrono::DateTime<chrono::Utc>,
    book_hash: Option<String>,
    meta_hash: Option<String>,
) -> Result<Vec<schema::BookNote>, sqlx::Error> {
    sqlx::query_as!(
        schema::BookNote,
        r#"
        SELECT *
        FROM book_notes
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

pub async fn upsert_book_notes(
    pool: &sqlx::PgPool,
    user_id: &uuid::Uuid,
    notes: Vec<schema::BookNote>,
) -> Result<Vec<schema::BookNote>, sqlx::Error> {
    let mut data = Vec::new();

    for note in notes {
        let record = sqlx::query_as!(
            schema::BookNote,
            r#"
            INSERT INTO public.book_notes ( user_id, book_hash, meta_hash, id, type, cfi, xpointer0, xpointer1, text, style, color, note, page, deleted_at )
            VALUES ( $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14 )
            ON CONFLICT (user_id, book_hash, id)
            DO UPDATE SET
              meta_hash  = EXCLUDED.meta_hash,
              type       = EXCLUDED.type,
              cfi        = EXCLUDED.cfi,
              xpointer0  = EXCLUDED.xpointer0,
              xpointer1  = EXCLUDED.xpointer1,
              text       = EXCLUDED.text,
              style      = EXCLUDED.style,
              color      = EXCLUDED.color,
              note       = EXCLUDED.note,
              page       = EXCLUDED.page,
              updated_at = now(),
              deleted_at = EXCLUDED.deleted_at
            RETURNING *
            "#,
            user_id,
            note.book_hash,
            note.meta_hash,
            note.id,
            note.r#type,
            note.cfi,
            note.xpointer0,
            note.xpointer1,
            note.text,
            note.style,
            note.color,
            note.note,
            note.page,
            note.deleted_at,
        )
        .fetch_one(pool)
        .await?;

        data.push(record);
    }

    Ok(data)
}
