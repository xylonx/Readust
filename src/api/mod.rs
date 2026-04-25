mod auth;
mod response;
pub mod state;
mod sync;
mod validator;

use axum::{Router, middleware};

pub fn router() -> Router {
    Router::new().merge(auth::router()).nest(
        "/api",
        Router::new()
            .merge(sync::router())
            .route_layer(middleware::from_fn(auth::auth_middleware)),
    )
}
