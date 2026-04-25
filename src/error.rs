#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Database error: {0}")]
    Sqlx(#[from] sqlx::Error),

    #[error("Axum extension extraction error: {0}")]
    Extension(#[from] axum::extract::rejection::ExtensionRejection),

    #[error("Query parameter extraction error: {0}")]
    Query(#[from] axum::extract::rejection::QueryRejection),

    #[error("Header extraction error: {0}")]
    Header(#[from] axum_extra::typed_header::TypedHeaderRejection),

    #[error("Invalid json payload: {0}")]
    Json(#[from] axum::extract::rejection::JsonRejection),

    #[error("Invalid JWT Token: {0}")]
    Jwt(#[from] jsonwebtoken::errors::Error),

    #[error("Authentication failed: {0}")]
    Unauthorized(String),

    #[error("Email already exists: {0}")]
    EmailAlreadyExists(String),

    #[error("Email does not exist: {0}")]
    EmailNotExist(String),

    #[error("Request validation failed: {0}")]
    Validated(#[from] validator::ValidationErrors),

    #[error("Invalid refresh Token: {0}")]
    InvalidRefreshToken(String),

    #[error("Invalid token id: token is expired or you have logout before")]
    InvalidTokenId,

    #[error("Signups not allowed for this instance")]
    SignupDisabled,

    #[error("Bcrypt error: {0}")]
    BcryptHash(#[from] bcrypt::BcryptError),

    #[error("Invalid login credentials")]
    InvalidPassword,

    #[error("Temp upload is not supported")]
    TempUploadUnsupported,

    #[error("Path contains unsupported characters")]
    MaliciousPathComponent,

    #[error("Failed to presign a s3 PutObject url: {0}")]
    S3PresignConfig(#[from] aws_sdk_s3::presigning::PresigningConfigError),

    #[error("Failed to presign a s3 PutObject url: {0}")]
    S3Sdk(Box<aws_sdk_s3::Error>),

    #[error("File not found")]
    FileNotFound,
}
