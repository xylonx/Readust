mod auth;
mod metrics;
mod response;
pub mod state;
mod storage;
mod sync;
mod validator;

use axum::{Router, middleware};

pub fn router() -> Router {
    Router::new().merge(auth::router()).nest(
        "/api",
        Router::new()
            .merge(sync::router())
            .merge(storage::router())
            .route_layer(middleware::from_fn(auth::auth_middleware))
            .route_layer(middleware::from_fn(metrics::metrics_middleware_fn)),
    )
}
