mod api;
mod db;
mod error;
mod settings;
mod utils;

use std::path::PathBuf;

use axum::{Extension, extract::Request};
use clap::Parser;
use config::Config;
use metrics_tracing_context::{MetricsLayer, TracingContextLayer};
use metrics_util::layers::Layer;
use tokio::signal;
use tower::ServiceBuilder;
use tower_http::{cors::CorsLayer, timeout::TimeoutLayer, trace::TraceLayer};
use tracing::{debug_span, error, info};
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

use crate::utils::{
    jwt::JwtClient, readust_metrics::setup_prometheus_metrics_recorder, s3::S3Client,
};

#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
struct CliArgs {
    #[arg(short, long)]
    config: Option<PathBuf>,
}

#[tokio::main]
async fn main() {
    let args = CliArgs::parse();

    let mut config_builder =
        Config::builder().add_source(config::Environment::with_prefix("READUST").separator("__"));
    if let Some(config_file) = args.config {
        config_builder = config_builder.add_source(config::File::from(config_file));
    }
    let setting = config_builder
        .build()
        .and_then(|c| c.try_deserialize::<settings::Settings>())
        .unwrap();

    let file_appender = tracing_appender::rolling::RollingFileAppender::builder()
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .max_log_files(setting.application.log_max_files)
        .filename_prefix(setting.application.log_file.to_str().unwrap())
        .build(setting.application.log_dir.to_str().unwrap())
        .unwrap();

    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    let console = fmt::Layer::new().with_writer(std::io::stdout).pretty();
    let inspector = fmt::Layer::new().with_writer(non_blocking).with_ansi(false);

    tracing_subscriber::registry()
        .with(console)
        .with(inspector)
        .with(MetricsLayer::new())
        .with(EnvFilter::from_default_env())
        .init();

    let pool = sqlx::PgPool::connect(&setting.database.uri).await.unwrap();

    sqlx::migrate!().run(&pool).await.unwrap();

    let jwt_client = JwtClient::new(
        &setting.application.jwt_secret,
        jsonwebtoken::Algorithm::HS256,
        setting.application.jwt_token_expires_in,
    );

    let s3_client = S3Client::new(setting.s3).await.unwrap();

    let state = api::state::AppState::new(
        pool,
        setting.application.anon_token,
        jwt_client,
        setting.application.disable_signup,
        s3_client,
    );

    let api = serve(
        state,
        setting.application.addr,
        setting.application.timeout.to_std().unwrap(),
    );

    let prometheus = serve_prometheus(
        setting.metrics.addr,
        setting.metrics.upkeep_duration.to_std().unwrap(),
    );

    tokio::select! {
        result = api => {
            if let Err(e) = result {
                error!(%e, "API server stopped");
            }
        },
        result = prometheus => {
            if let Err(e) = result {
                error!(%e, "Prometheus server stopped");
            }
        }
    }
}

async fn serve(
    state: api::state::AppState,
    addr: String,
    timeout: std::time::Duration,
) -> Result<(), std::io::Error> {
    let api_router = api::router();

    let app = api_router.layer(
        ServiceBuilder::new()
            .layer(Extension(state))
            .layer(TraceLayer::new_for_http().make_span_with(|req: &Request| {
                debug_span!(
                    "request",
                    method = %req.method(),
                    uri = %req.uri(),
                    path = req.uri().path(),
                    version = ?req.version(),
                    trace_id = %uuid::Uuid::new_v4(),
                    user_id = tracing::field::Empty,
                )
            }))
            .layer(TimeoutLayer::with_status_code(
                axum::http::StatusCode::REQUEST_TIMEOUT,
                timeout,
            ))
            .layer(CorsLayer::permissive()),
    );

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    info!("Listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
}

async fn serve_prometheus(
    addr: String,
    upkeep_duration: std::time::Duration,
) -> Result<(), std::io::Error> {
    let recorder = setup_prometheus_metrics_recorder().unwrap();
    let handle = recorder.handle();

    let recorder = TracingContextLayer::all().layer(recorder);

    metrics::set_global_recorder(recorder).unwrap();

    let upkeep_handle = handle.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(upkeep_duration).await;
            upkeep_handle.run_upkeep();
        }
    });

    let router = axum::Router::new().route(
        "/metrics",
        axum::routing::get(move || std::future::ready(handle.render())),
    );

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    info!("prometheus listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler")
    };

    #[cfg(unix)]
    let terminate = async {
        use tokio::signal;

        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {}
    }
}
