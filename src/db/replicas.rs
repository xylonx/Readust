use crate::db::schema::{ReplicaKind, ReplicaRow};

pub async fn get_replicas(
    pool: &sqlx::PgPool,
    user_id: &uuid::Uuid,
    kind: &ReplicaKind,
    since: Option<&String>,
) -> Result<Vec<ReplicaRow>, sqlx::Error> {
    sqlx::query_as!(
        ReplicaRow,
        r#"
        SELECT
            user_id,
            kind AS "kind: ReplicaKind",
            replica_id,
            fields_jsonb,
            manifest_jsonb,
            deleted_at_ts,
            reincarnation,
            updated_at_ts,
            schema_version,
            created_at,
            modified_at
        FROM replicas
        WHERE user_id = $1 AND kind = $2 AND ($3::text IS NULL OR updated_at_ts > $3)
        ORDER BY updated_at_ts
        LIMIT 1000
        "#,
        user_id,
        kind.as_str(),
        since,
    )
    .fetch_all(pool)
    .await
}

pub async fn merge_replicas(
    pool: &sqlx::PgPool,
    row: &ReplicaRow,
) -> Result<ReplicaRow, sqlx::Error> {
    sqlx::query_as!(
        ReplicaRow,
        r#"
        SELECT
            user_id       AS "user_id!",
            kind          AS "kind!: ReplicaKind",
            replica_id    AS "replica_id!",
            fields_jsonb  AS "fields_jsonb!",
            manifest_jsonb,
            deleted_at_ts,
            reincarnation,
            updated_at_ts AS "updated_at_ts!",
            schema_version AS "schema_version!",
            created_at    AS "created_at!",
            modified_at   AS "modified_at!"
        FROM crdt_merge_replica($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#,
        row.user_id,
        row.kind.as_str(),
        row.replica_id,
        row.fields_jsonb,
        row.manifest_jsonb,
        row.deleted_at_ts,
        row.reincarnation,
        row.updated_at_ts,
        row.schema_version
    )
    .fetch_one(pool)
    .await
}
