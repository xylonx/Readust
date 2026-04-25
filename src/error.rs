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
    JWT(#[from] jsonwebtoken::errors::Error),

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
}
