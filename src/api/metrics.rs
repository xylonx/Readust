use axum::{extract::Request, middleware::Next, response::IntoResponse};
use metrics::{counter, gauge, histogram};

use crate::utils::readust_metrics::{
    HTTP_REQUEST_DURATION_SECONDS, HTTP_REQUESTS_IN_FLIGHT, HTTP_REQUESTS_TOTAL,
};

pub async fn metrics_middleware_fn(req: Request, next: Next) -> impl IntoResponse {
    counter!(HTTP_REQUESTS_TOTAL.name).increment(1);

    let start_at = chrono::Utc::now();

    gauge!(HTTP_REQUESTS_IN_FLIGHT.name).increment(1);
    let resp = next.run(req).await;
    gauge!(HTTP_REQUESTS_IN_FLIGHT.name).decrement(1);

    let end_at = chrono::Utc::now();

    histogram!(HTTP_REQUEST_DURATION_SECONDS.name).record((end_at - start_at).as_seconds_f64());

    resp
}
