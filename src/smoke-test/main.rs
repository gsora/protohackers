use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};

#[tokio::main]
async fn main() {
    let port = std::env::args().nth(1).unwrap_or("9999".to_string());

    let bind_addr = format!("0.0.0.0:{}", port);

    let listener = TcpListener::bind(bind_addr.clone()).await.unwrap();

    println!("listening on {}", bind_addr);

    loop {
        let mut socket = match listener.accept().await {
            Ok((socket, _)) => socket,
            Err(error) => {
                eprintln!("error accepting connection: {:?}", error);

                continue;
            }
        };

        println!("accepted connection");

        handle_smoke_test(&mut socket).await;
    }
}

async fn handle_smoke_test(socket: &mut TcpStream) {
    let mut data = vec![];

    match socket.read_to_end(&mut data).await {
        Ok(_) => (),
        Err(error) => {
            eprintln!("error reading data from socket: {:?}", error);
            return;
        }
    }

    match socket.write_all(&data).await {
        Ok(_) => (),
        Err(error) => {
            eprintln!("error writing data to socket: {:?}", error);
            return;
        }
    }
}
