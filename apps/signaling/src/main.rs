#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "signaling=info".into()),
        )
        .init();

    let port: u16 = std::env::var("REMOTEX_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(protocol::DEFAULT_SIGNALING_PORT);
    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
    signaling::serve(addr).await.expect("serve");
}
