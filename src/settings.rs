#[derive(Debug, serde::Deserialize)]
pub struct Settings {
    pub application: ApplicationSettings,

    pub database: DBSettings,
}

#[derive(Debug, serde::Deserialize)]
pub struct ApplicationSettings {
    pub addr: String,

    pub timeout_secs: u64,

    pub jwt_secret: String,
    pub jwt_token_expires: i64,

    pub anon_token: String,

    pub disable_signup: bool,

    pub log_dir: std::path::PathBuf,
    pub log_file: std::path::PathBuf,
}

#[derive(Debug, serde::Deserialize)]
pub struct DBSettings {
    pub uri: String,
}
