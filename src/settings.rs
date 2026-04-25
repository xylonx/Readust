use crate::utils::{s3::S3Settings, serdefmt::duration_seconds};

#[derive(Debug, serde::Deserialize)]
pub struct Settings {
    pub application: ApplicationSettings,

    pub database: DBSettings,

    pub s3: S3Settings,
}

#[derive(Debug, serde::Deserialize)]
pub struct ApplicationSettings {
    pub addr: String,

    #[serde(with = "duration_seconds")]
    pub timeout: chrono::Duration,

    pub jwt_secret: String,
    #[serde(with = "duration_seconds")]
    pub jwt_token_expires_in: chrono::Duration,

    pub anon_token: String,

    pub disable_signup: bool,

    pub log_dir: std::path::PathBuf,
    pub log_file: std::path::PathBuf,
}

#[derive(Debug, serde::Deserialize)]
pub struct DBSettings {
    pub uri: String,
}
