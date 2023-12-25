use tokio::{net::{TcpListener, TcpStream}, sync::mpsc, io::{AsyncWriteExt, AsyncBufReadExt}};

/// Returns a TcpListener that binds on 0.0.0.0, either on port 9999
/// or on whatever was passed as first CLI argument.
pub async fn bind_listener() -> TcpListener {
    let port = std::env::args().nth(1).unwrap_or("9999".to_string());

    let bind_addr = format!("0.0.0.0:{}", port);

    let listener = TcpListener::bind(bind_addr.clone()).await.unwrap();

    println!("listening on {}", bind_addr);

    listener
}

/// Returns a channel for writing and reading off a TcpStream.
/// Useful to use a TcpStream in a concurrent manner.
pub fn socket_rxtx(stream: TcpStream) -> (mpsc::Sender<String>, mpsc::Receiver<Option<String>>) {
    let (user_sender, mut receiver) = mpsc::channel::<String>(1);
    let (sender, user_receiver) = mpsc::channel::<Option<String>>(1);

    let std_stream = stream.into_std().unwrap();

    let stream_recv = std_stream.try_clone().unwrap();
    tokio::spawn(async move {
        let mut stream = TcpStream::from_std(stream_recv).unwrap();

        loop {
            if let Ok(recv) = receiver.try_recv() {
                log::debug!("received data to send to socket: {}", recv.trim());
                stream.write_all(recv.as_bytes()).await.unwrap();
            }
        }
    });

    let stream_recv = std_stream.try_clone().unwrap();
    tokio::spawn(async move {
        let stream = TcpStream::from_std(stream_recv).unwrap();
        let mut reader = tokio::io::BufReader::new(stream);

        loop {
            log::debug!("socket loop write");

            let mut sent_message = String::new();

            match reader.read_line(&mut sent_message).await {
                Ok(_) => sender.send(Some(sent_message)).await.unwrap(),
                Err(_) => sender.send(None).await.unwrap(),
            }
        }
    });

    (user_sender, user_receiver)
}