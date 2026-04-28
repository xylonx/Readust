use std::str::FromStr;

use crate::utils::{s3::S3Settings, serdefmt::duration_seconds};

#[derive(Debug, serde::Deserialize)]
pub struct Settings {
    pub application: ApplicationSettings,

    pub metrics: Option<MetricsSettings>,

    pub database: DBSettings,

    pub s3: S3Settings,
}

#[derive(Debug, serde::Deserialize)]
pub struct ApplicationSettings {
    #[serde(default = "default_addr")]
    pub addr: String,

    #[serde(with = "duration_seconds", default = "default_timout")]
    pub timeout: chrono::Duration,

    pub jwt_secret: String,
    #[serde(with = "duration_seconds", default = "default_jwt_token_expires_in")]
    pub jwt_token_expires_in: chrono::Duration,

    pub anon_token: String,

    #[serde(default)]
    pub disable_signup: bool,

    #[serde(default = "default_log_dir")]
    pub log_dir: std::path::PathBuf,
    #[serde(default = "default_log_file")]
    pub log_file: std::path::PathBuf,
    #[serde(default = "default_log_max_files")]
    pub log_max_files: usize,
}

fn default_addr() -> String {
    "0.0.0.0:8000".to_string()
}

fn default_timout() -> chrono::Duration {
    chrono::Duration::seconds(120)
}

fn default_jwt_token_expires_in() -> chrono::Duration {
    chrono::Duration::seconds(3600)
}

fn default_log_dir() -> std::path::PathBuf {
    std::path::PathBuf::from_str("./logs/").unwrap()
}

fn default_log_file() -> std::path::PathBuf {
    std::path::PathBuf::from_str("app.log").unwrap()
}

fn default_log_max_files() -> usize {
    14
}

#[derive(Debug, serde::Deserialize)]
pub struct MetricsSettings {
    #[serde(default)]
    pub enable: bool,

    #[serde(default = "default_metrics_addr")]
    pub addr: String,

    #[serde(default = "default_upkeep_duration")]
    pub upkeep_duration: chrono::Duration,
}

fn default_metrics_addr() -> String {
    "0.0.0.0:8080".to_string()
}

fn default_upkeep_duration() -> chrono::Duration {
    chrono::Duration::seconds(10)
}

#[derive(Debug, serde::Deserialize)]
pub struct DBSettings {
    pub uri: String,
}
