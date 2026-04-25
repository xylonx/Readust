use axum::{
    Extension, Json, RequestExt, Router,
    extract::{FromRequest, FromRequestParts, Query, Request},
    http::StatusCode,
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use axum_extra::{
    TypedHeader,
    headers::{Authorization, authorization::Bearer},
};
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info};
use validator::Validate;

use crate::{
    api::{
        response::ApiResult,
        state::{AppState, AuthState, AuthStateExtractor, StateExtractor},
        validator::ValidatedJson,
    },
    db::{self, schema},
    error::Error,
    utils::serdefmt::duration_ms,
};

#[derive(Debug, FromRequestParts)]
#[from_request(rejection(Error))]
struct AuthHeaderExtractor {
    state: Extension<AppState>,
    bearer: TypedHeader<Authorization<Bearer>>,
}

async fn extract_auth_state(req: &mut Request) -> ApiResult<AuthState> {
    let AuthHeaderExtractor { state, bearer } = req.extract_parts::<AuthHeaderExtractor>().await?;
    let token = bearer.token();
    if token.is_empty() {
        return Err(Error::Unauthorized("Bearer token is empty".to_string()));
    }

    let claims = state.jwt_client.validate_token(token)?;

    db::token::get_token_by_id(&state.pool, &claims.token_id)
        .await
        .map_err(|_| Error::InvalidTokenId)?;

    match db::user::get_user_by_id(&state.pool, &claims.user_id).await? {
        Some(user) => Ok(AuthState {
            user,
            token_id: claims.token_id,
        }),

        None => Err(Error::Unauthorized(format!(
            "User does not exist: {}",
            claims.user_id
        ))),
    }
}

async fn extract_anon_auth_state(req: &mut Request) -> ApiResult<()> {
    let AuthHeaderExtractor { state, bearer } = req.extract_parts::<AuthHeaderExtractor>().await?;
    let token = bearer.token();

    if token != state.anon_token {
        Err(Error::Unauthorized("Invalid anon auth token".to_string()))
    } else {
        Ok(())
    }
}

pub async fn auth_middleware(mut req: Request, next: Next) -> ApiResult<Response> {
    let auth_state = extract_auth_state(&mut req).await?;
    info!("Insert auth state extension");
    req.extensions_mut().insert(auth_state);
    Ok(next.run(req).await)
}

pub fn router() -> Router {
    Router::new().nest(
        "/auth/v1",
        Router::new()
            .merge(
                Router::new()
                    .route("/user", get(get_user))
                    .route("/logout", post(logout))
                    .route_layer(middleware::from_fn(auth_middleware)),
            )
            .merge(
                Router::new()
                    .route("/token", post(token))
                    .route("/signup", post(signup)),
            ),
    )
}

#[derive(Debug, Deserialize, Validate)]
struct SignupPayload {
    #[validate(email)]
    email: String,
    #[validate(length(min = 8))]
    password: String,
}

async fn signup(
    StateExtractor { state }: StateExtractor,
    ValidatedJson(data): ValidatedJson<SignupPayload>,
) -> ApiResult<Json<schema::User>> {
    if state.disable_signup {
        return Err(Error::SignupDisabled);
    }

    match db::user::get_user_by_email(&state.pool, &data.email).await? {
        Some(_) => Err(Error::EmailAlreadyExists(data.email)),
        None => {
            let hashed = bcrypt::hash(data.password, bcrypt::DEFAULT_COST)?;
            let user = db::user::insert_user(&state.pool, &data.email, &hashed).await?;

            Ok(Json(user))
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum TokenGrantKind {
    Password,
    RefreshToken,
}

#[derive(Debug, Deserialize)]
struct TokenQuery {
    grant_type: TokenGrantKind,
}

#[derive(Debug, Deserialize, Validate)]
struct TokenPassword {
    #[validate(email)]
    email: String,
    password: String,
}

#[derive(Debug, Deserialize)]
struct TokenRefreshToken {
    refresh_token: uuid::Uuid,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
enum AccessTokenKind {
    Bearer,
}

#[derive(Debug, Serialize)]
struct AccessTokenResponse {
    access_token: String,
    token_type: AccessTokenKind,
    #[serde(with = "duration_ms")]
    expires_in: chrono::Duration,
    #[serde(with = "chrono::serde::ts_seconds")]
    expires_at: chrono::DateTime<chrono::Utc>,
    refresh_token: uuid::Uuid,
    user: schema::User,
}

#[derive(Debug, FromRequestParts)]
#[from_request(rejection(Error))]
struct TokenExetractor {
    #[from_request(via(Query))]
    query: TokenQuery,
}

async fn token(
    StateExtractor { state }: StateExtractor,
    TokenExetractor { query }: TokenExetractor,
    mut req: Request,
) -> ApiResult<Json<AccessTokenResponse>> {
    match query.grant_type {
        TokenGrantKind::Password => {
            let ValidatedJson(data) = req.extract::<ValidatedJson<TokenPassword>, _>().await?;
            match db::user::get_user_by_email(&state.pool, &data.email).await? {
                Some(user) => {
                    if !bcrypt::verify(data.password, &user.encrypted_password)? {
                        error!(user.email, "Failed to verfiy password");
                        Err(Error::InvalidPassword)
                    } else {
                        Ok(Json(generate_token(state, user).await?))
                    }
                }
                None => {
                    error!(%data.email, "User does not exist");
                    Err(Error::EmailNotExist(data.email))
                }
            }
        }
        TokenGrantKind::RefreshToken => {
            // Readest use Supabase which has a strange logic. When refreshing the access token,
            // it will use the anon bearer token instead of the existing access token.
            // Therefore, we have to check whether token is the anon one first.
            if extract_anon_auth_state(&mut req).await.is_ok() {
                let data = Json::<TokenRefreshToken>::from_request(req, &()).await?;
                let token =
                    db::token::delete_token_by_refresh_token(&state.pool, &data.refresh_token)
                        .await?;
                let user = db::user::get_user_by_id(&state.pool, &token.user_id)
                    .await?
                    .ok_or_else(|| {
                        Error::Unauthorized(format!("user does not exist: {}", token.user_id))
                    })?;

                return Ok(Json(generate_token(state, user).await?));
            }

            // If the bearer token is not anon, we will then treat the bearer token as presigned one
            let auth = extract_auth_state(&mut req).await?;
            let data = Json::<TokenRefreshToken>::from_request(req, &()).await?;

            let token = db::token::get_token_by_id(&state.pool, &auth.token_id).await?;
            if token.user_id != auth.user.id {
                error!(%token.user_id, %auth.user.id, "UserID does not match");
                return Err(Error::InvalidRefreshToken(format!(
                    "Token is generated for user {} but used by {}",
                    token.user_id, auth.user.id
                )));
            }
            if token.refresh_token != data.refresh_token {
                error!(%data.refresh_token, "Invalid refresh token");
                return Err(Error::InvalidRefreshToken(
                    "refresh token does not match".to_string(),
                ));
            }

            db::token::delete_token(&state.pool, &auth.token_id).await?;

            Ok(Json(generate_token(state, auth.user).await?))
        }
    }
}

async fn generate_token(state: AppState, user: schema::User) -> ApiResult<AccessTokenResponse> {
    let claims = state.jwt_client.generate_claims(user.id);
    let jwt_token = state.jwt_client.generate_jwt_token(&claims)?;
    let refresh_token = state.jwt_client.new_refresh_token();
    let token = db::token::create_token(
        &state.pool,
        claims.token_id,
        refresh_token,
        user.id,
        claims.expires_at,
    )
    .await?;
    debug!(?token, "Generate token");
    Ok(AccessTokenResponse {
        access_token: jwt_token,
        token_type: AccessTokenKind::Bearer,
        expires_in: state.jwt_client.expires_duration(),
        expires_at: claims.expires_at,
        refresh_token: refresh_token,
        user,
    })
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename = "lowercase")]
enum LogoutScope {
    #[default]
    Global,
    Local,
}

#[derive(Debug, Deserialize)]
struct LogoutQuery {
    #[serde(default)]
    scope: LogoutScope,
}

#[derive(Debug, FromRequestParts)]
#[from_request(rejection(Error))]
struct LogoutExtractor {
    #[from_request(via(Query))]
    query: LogoutQuery,
}

async fn logout(
    AuthStateExtractor { state, auth }: AuthStateExtractor,
    LogoutExtractor { query }: LogoutExtractor,
) -> ApiResult<impl IntoResponse> {
    match query.scope {
        LogoutScope::Global => {
            db::token::delete_token_by_user_id(&state.pool, &auth.user.id).await?
        }
        LogoutScope::Local => db::token::delete_token(&state.pool, &auth.token_id).await?,
    };
    Ok(StatusCode::NO_CONTENT)
}

async fn get_user(
    AuthStateExtractor { state: _, auth }: AuthStateExtractor,
) -> ApiResult<Json<schema::User>> {
    Ok(Json(auth.user))
}
