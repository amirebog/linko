use linko::config::Config;
use linko::db;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    if let Err(error) = run().await {
        tracing::error!("{error}");
        eprintln!("{error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("bootstrap-admin") => {
            let username = args
                .next()
                .ok_or_else(|| "Usage: linko bootstrap-admin <username>".to_string())?;
            bootstrap_admin_cli(&username).await
        }
        Some(other) => Err(format!("Unknown command: {other}")),
        None => serve().await,
    }
}

async fn bootstrap_admin_cli(username: &str) -> Result<(), String> {
    let mut config = Config::from_env()?;
    config.bootstrap_admin = Some(username.trim().to_ascii_lowercase());
    let pool = db::init_pool(&config.database_path).await?;
    db::bootstrap_admin(&pool, &config).await?;
    pool.close().await;
    println!("Bootstrap complete for `{username}` (only if no admin existed).");
    Ok(())
}

async fn serve() -> Result<(), String> {
    let config = Config::from_env()?;
    let state = linko::build_state(config.clone()).await?;
    let addr: SocketAddr = config
        .bind_addr()
        .parse()
        .map_err(|e| format!("Invalid bind address: {e}"))?;
    let listener = TcpListener::bind(addr)
        .await
        .map_err(|e| format!("Failed to bind {addr}: {e}"))?;
    let actual = listener
        .local_addr()
        .map_err(|e| format!("Failed to read bind address: {e}"))?;
    println!("Linko is running at http://{actual}");
    tracing::info!("listening on http://{actual}");

    axum::serve(
        listener,
        linko::router(state.clone()).into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await
    .map_err(|e| format!("Server error: {e}"))?;

    state.pool.close().await;
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = tokio::signal::ctrl_c();
    #[cfg(unix)]
    {
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("install SIGTERM handler");
        tokio::select! {
            _ = ctrl_c => {}
            _ = terminate.recv() => {}
        }
    }
    #[cfg(not(unix))]
    {
        let _ = ctrl_c.await;
    }
}
