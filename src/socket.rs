use tokio::net::TcpListener;

/// Returns a TcpListener that binds on 0.0.0.0, either on port 9999
/// or on whatever was passed as first CLI argument.
pub async fn bind_listener() -> TcpListener {
    let port = std::env::args().nth(1).unwrap_or("9999".to_string());

    let bind_addr = format!("0.0.0.0:{}", port);

    let listener = TcpListener::bind(bind_addr.clone()).await.unwrap();

    println!("listening on {}", bind_addr);

    listener
}
