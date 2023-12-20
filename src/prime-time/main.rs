use protohackers::socket::bind_listener;
use serde::{Deserialize, Serialize};
use std::env::set_var;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

#[derive(Serialize, Deserialize, Debug)]
enum MessageMethod {
    #[serde(rename = "isPrime")]
    IsPrime,
}

#[derive(Serialize, Deserialize, Debug)]
struct Message {
    method: MessageMethod,
    number: f64,
}

#[derive(Serialize, Deserialize, Debug)]
struct MessageResponse {
    method: MessageMethod,
    prime: bool,
}

#[tokio::main]
async fn main() {
    set_var("RUST_LOG", "debug");
    env_logger::init();

    let listener = bind_listener().await;

    loop {
        let socket = match listener.accept().await {
            Ok((socket, _)) => socket,
            Err(error) => {
                log::error!("error accepting connection: {:?}", error);
                continue;
            }
        };

        log::info!("accepted connection");

        tokio::spawn(async move {
            handle_prime_time(socket).await;
        });
    }
}

async fn handle_prime_time(stream: TcpStream) {
    let reader = tokio::io::BufReader::new(stream);

    let mut lines = reader.lines();

    while let Some(line) = match lines.next_line().await {
        Ok(maybe_line) => maybe_line,
        Err(error) => {
            log::error!("can't read line: {}", error);
            return;
        }
    } {
        log::debug!("unmarshaling message");
        let msg: Message = match serde_json::from_str(&line) {
            Ok(msg) => msg,
            Err(error) => {
                log::warn!("cannot unmarshal message: {:?}", error);
                match lines.get_mut().write_all("{}\n".as_bytes()).await {
                    Ok(_) => (),
                    Err(error) => {
                        log::error!("error writing data to socket: {:?}", error);
                        return;
                    }
                }

                log::debug!("returned malformed response");

                continue;
            }
        };

        let is_prime = primes::is_prime(msg.number as u64);

        let response = MessageResponse {
            method: MessageMethod::IsPrime,
            prime: is_prime,
        };

        let mut msg_bytes = serde_json::to_vec(&response).unwrap();
        msg_bytes.push(b'\n');

        match lines.get_mut().write_all(&msg_bytes).await {
            Ok(_) => (),
            Err(error) => {
                log::error!("error writing data to socket: {:?}", error);
                return;
            }
        }

        log::debug!("answer returned");
    }
}
