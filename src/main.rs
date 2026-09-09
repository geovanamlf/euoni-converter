use euoni_converter::{app::create_app, config::Settings};
use std::net::SocketAddr;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let settings = match Settings::load() {
        Ok(settings) => settings,
        Err(err) => {
            tracing::error!(error = %err, "failed to load settings");
            std::process::exit(1);
        }
    };

    let addr: SocketAddr = format!("{}:{}", settings.host, settings.port)
        .parse()
        .expect("bind address must be valid");

    let app = create_app(settings.clone());

    tracing::info!(host = %settings.host, port = settings.port, "starting local converter server");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind TCP listener");

    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            tokio::signal::ctrl_c()
                .await
                .expect("failed to install ctrl-c handler");
        })
        .await
        .expect("server failed");
}
