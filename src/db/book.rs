use crate::db::schema;

pub async fn get_books(
    pool: &sqlx::PgPool,
    user_id: &uuid::Uuid,
    since: chrono::DateTime<chrono::Utc>,
    book_hash: Option<String>,
    meta_hash: Option<String>,
) -> Result<Vec<schema::Book>, sqlx::Error> {
    sqlx::query_as!(
        schema::Book,
        r#"
        SELECT *
        FROM books
        WHERE user_id = $1
          AND (updated_at > $2 OR deleted_at > $2)
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

pub async fn upsert_books(
    pool: &sqlx::PgPool,
    user_id: &uuid::Uuid,
    books: Vec<schema::Book>,
) -> Result<Vec<schema::Book>, sqlx::Error> {
    let mut data = Vec::new();

    for book in books {
        let upserted = sqlx::query_as!(
            schema::Book,
            r#"
            INSERT INTO public.books ( user_id, book_hash, meta_hash, format, title, source_title, author, "group", tags, created_at, updated_at, deleted_at, uploaded_at, progress, reading_status, group_id, group_name, metadata )
            VALUES ( $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, NOW(), $12, $13, $14, $15, $16, $17, $18 )
            ON CONFLICT (user_id, book_hash)
            DO UPDATE SET
              meta_hash      = EXCLUDED.meta_hash,
              format         = EXCLUDED.format,
              title          = EXCLUDED.title,
              source_title   = EXCLUDED.source_title,
              author         = EXCLUDED.author,
              "group"        = EXCLUDED."group",
              tags           = EXCLUDED.tags,
              updated_at     = $11,
              deleted_at     = EXCLUDED.deleted_at,
              uploaded_at    = EXCLUDED.uploaded_at,
              progress       = EXCLUDED.progress,
              reading_status = EXCLUDED.reading_status,
              group_id       = EXCLUDED.group_id,
              group_name     = EXCLUDED.group_name,
              metadata       = EXCLUDED.metadata
            WHERE $11 > books.updated_at
               OR COALESCE(EXCLUDED.deleted_at, 'epoch'::timestamptz)
                  > COALESCE(books.deleted_at, 'epoch'::timestamptz)
            RETURNING *
            "#,
            user_id,
            &book.book_hash,
            book.meta_hash,
            book.format,
            book.title,
            book.source_title,
            book.author,
            book.group,
            book.tags.as_deref(),
            book.created_at,
            book.updated_at,
            book.deleted_at,
            book.uploaded_at,
            book.progress.as_deref(),
            book.reading_status,
            book.group_id,
            book.group_name,
            book.metadata,
        )
        .fetch_optional(pool)
        .await?;

        // A rejected update returns no row; read the current server version.
        let winner = match upserted {
            Some(book) => book,
            None => sqlx::query_as!(
                schema::Book,
                r#"
                SELECT *
                FROM books
                WHERE user_id = $1
                  AND book_hash = $2
                "#,
                user_id,
                &book.book_hash,
            )
            .fetch_one(pool)
            .await?,
        };

        data.push(winner);
    }

    Ok(data)
}
