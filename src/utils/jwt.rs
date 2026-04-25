use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use tracing::{debug, instrument};

use crate::error::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    #[serde(rename = "iss")]
    issuer: String,
    #[serde(rename = "sub")]
    pub user_id: uuid::Uuid,
    #[serde(rename = "aud")]
    audience: Option<String>,
    #[serde(rename = "exp", with = "chrono::serde::ts_seconds")]
    pub expires_at: chrono::DateTime<chrono::Utc>,
    #[serde(rename = "iat", with = "chrono::serde::ts_seconds")]
    issued_at: chrono::DateTime<chrono::Utc>,
    #[serde(rename = "nbf", with = "chrono::serde::ts_seconds")]
    not_before: chrono::DateTime<chrono::Utc>,
    #[serde(rename = "jti")]
    pub token_id: uuid::Uuid,
}

#[derive(Debug)]
pub struct JwtClient {
    secret: Vec<u8>,
    algorithm: Algorithm,
    token_expires_in: chrono::Duration,
}

impl JwtClient {
    pub fn new(secret: &str, algorithm: Algorithm, token_expires_in: chrono::Duration) -> Self {
        Self {
            secret: secret.as_bytes().to_owned(),
            algorithm,
            token_expires_in,
        }
    }

    pub fn new_token_id(&self) -> uuid::Uuid {
        uuid::Uuid::new_v4()
    }

    pub fn new_refresh_token(&self) -> uuid::Uuid {
        uuid::Uuid::new_v4()
    }

    pub fn expires_duration(&self) -> chrono::Duration {
        self.token_expires_in
    }

    #[instrument(skip(self))]
    pub fn generate_claims(&self, user_id: uuid::Uuid) -> Claims {
        let issued_at = chrono::Utc::now();
        let token_id = self.new_token_id();
        let expires_at = issued_at + self.token_expires_in;
        Claims {
            issuer: "Readust".to_string(),
            user_id,
            audience: None,
            expires_at,
            issued_at,
            not_before: issued_at,
            token_id,
        }
    }

    #[instrument(skip(self))]
    pub fn generate_jwt_token(&self, claims: &Claims) -> Result<String, Error> {
        Ok(encode(
            &Header::new(self.algorithm),
            claims,
            &EncodingKey::from_secret(&self.secret),
        )?)
    }

    #[instrument(skip_all)]
    pub fn validate_token(&self, token: &str) -> Result<Claims, Error> {
        let mut validation = Validation::new(self.algorithm);

        validation.set_issuer(&["Readust".to_string()]);
        validation.set_required_spec_claims(&["iss", "exp", "iat", "nbt"]);
        let claims = decode::<Claims>(token, &DecodingKey::from_secret(&self.secret), &validation)?;

        debug!(
            subject = claims.claims.user_id.to_string(),
            "decode claims successfully"
        );

        Ok(claims.claims)
    }
}
