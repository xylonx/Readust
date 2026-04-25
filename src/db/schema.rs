#![allow(dead_code)]
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::utils::serdefmt::{
    opt_json_obj_in_str_out, option_timestamp_mix_ts_str, timestamp_mix_ts_str,
};

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    #[serde(skip)]
    pub encrypted_password: String,
    pub created_at: DateTime<Utc>,
    #[serde(with = "chrono::serde::ts_milliseconds", default)]
    pub updated_at: DateTime<Utc>,
    #[serde(skip)]
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Token {
    pub id: Uuid,
    pub user_id: Uuid,
    pub refresh_token: Uuid,
    pub expires_at: DateTime<Utc>,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub created_at: DateTime<Utc>,
    #[serde(with = "chrono::serde::ts_milliseconds", default)]
    pub updated_at: DateTime<Utc>,
    #[serde(skip)]
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all(serialize = "snake_case", deserialize = "camelCase"))]
pub struct Book {
    #[serde(default)]
    pub user_id: Uuid,
    #[serde(alias = "hash")]
    pub book_hash: String,
    pub meta_hash: Option<String>,
    pub format: Option<String>,
    pub title: Option<String>,
    pub source_title: Option<String>,
    pub author: Option<String>,
    pub group: Option<String>,
    pub tags: Option<Vec<String>>,
    #[serde(with = "timestamp_mix_ts_str", default)]
    pub created_at: DateTime<Utc>,
    #[serde(with = "timestamp_mix_ts_str", default)]
    pub updated_at: DateTime<Utc>,
    #[serde(with = "option_timestamp_mix_ts_str")]
    pub deleted_at: Option<DateTime<Utc>>,
    pub uploaded_at: Option<DateTime<Utc>>,
    pub progress: Option<Vec<i32>>,
    pub reading_status: Option<String>,
    pub group_id: Option<String>,
    pub group_name: Option<String>,
    #[serde(with = "opt_json_obj_in_str_out")]
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all(serialize = "snake_case", deserialize = "camelCase"))]
pub struct BookConfig {
    #[serde(default)]
    pub user_id: Uuid,
    #[serde(alias = "hash")]
    pub book_hash: String,
    pub meta_hash: Option<String>,
    pub location: Option<String>,
    pub xpointer: Option<String>,
    #[serde(with = "opt_json_obj_in_str_out")]
    pub progress: Option<serde_json::Value>,
    pub rsvp_position: Option<String>,
    #[serde(with = "opt_json_obj_in_str_out")]
    pub search_config: Option<serde_json::Value>,
    #[serde(with = "opt_json_obj_in_str_out")]
    pub view_settings: Option<serde_json::Value>,
    #[serde(with = "timestamp_mix_ts_str", default)]
    pub created_at: DateTime<Utc>,
    #[serde(with = "timestamp_mix_ts_str", default)]
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all(serialize = "snake_case", deserialize = "camelCase"))]
pub struct BookNote {
    #[serde(default)]
    pub user_id: Uuid,
    #[serde(alias = "hash")]
    pub book_hash: String,
    pub meta_hash: Option<String>,
    pub id: String,
    pub r#type: Option<String>,
    pub cfi: Option<String>,
    pub xpointer0: Option<String>,
    pub xpointer1: Option<String>,
    pub text: Option<String>,
    pub style: Option<String>,
    pub color: Option<String>,
    pub note: Option<String>,
    pub page: Option<i32>,
    #[serde(with = "timestamp_mix_ts_str", default)]
    pub created_at: DateTime<Utc>,
    #[serde(with = "timestamp_mix_ts_str", default)]
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all(serialize = "snake_case", deserialize = "camelCase"))]
pub struct File {
    pub id: Uuid,
    #[serde(default)]
    pub user_id: Uuid,
    #[serde(alias = "hash")]
    pub book_hash: Option<String>,
    pub file_key: String,
    pub file_size: i64,
    #[serde(with = "option_timestamp_mix_ts_str", default)]
    pub created_at: Option<DateTime<Utc>>,
    #[serde(with = "option_timestamp_mix_ts_str", default)]
    pub updated_at: Option<DateTime<Utc>>,
    pub deleted_at: Option<DateTime<Utc>>,
}
