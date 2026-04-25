use axum::{Json, http::StatusCode, response::IntoResponse};
use serde_json::json;

use crate::error::Error;

pub type ApiResult<T> = Result<T, Error>;

impl Error {
    fn status_code(&self) -> StatusCode {
        match self {
            Error::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            Error::EmailAlreadyExists(_) => StatusCode::CONFLICT,
            Error::SignupDisabled => StatusCode::UNPROCESSABLE_ENTITY,
            Error::Sqlx(_) | Error::Extension(_) | Error::BcryptHash(_) => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
            _ => StatusCode::BAD_REQUEST,
        }
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> axum::response::Response {
        let code = self.status_code();
        let msg = self.to_string();
        (code, Json(json!({ "error": msg }))).into_response()
    }
}
