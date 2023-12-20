use byteorder::BigEndian;
use byteorder::ByteOrder;
use std::collections::HashMap;
use std::env::set_var;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::net::TcpStream;

#[derive(Debug)]
struct InsertMessage {
    timestamp: i32,
    price: i32,
}

#[derive(Debug)]
struct QueryMessage {
    min_time: i32,
    max_time: i32,
}

#[derive(Debug)]
enum Message {
    Insert(InsertMessage),
    Query(QueryMessage),
    Undefined,
}

type RawMessage = [u8; 9];
impl From<RawMessage> for Message {
    fn from(value: RawMessage) -> Self {
        let typ = value[0] as char;

        let value = &value[1..];

        let first = &value[0..4];
        let second = &value[4..];

        match typ {
            'I' => Self::Insert(InsertMessage {
                timestamp: BigEndian::read_i32(first),
                price: BigEndian::read_i32(second),
            }),
            'Q' => Self::Query(QueryMessage {
                min_time: BigEndian::read_i32(first),
                max_time: BigEndian::read_i32(second),
            }),

            _ => Self::Undefined,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn parse_test() {
        let msg: RawMessage = [b'I', 0, 0, 0, 1, 0, 0, 0, 1];
        assert_eq!(
            true,
            matches!(
                Message::from(msg),
                Message::Insert(InsertMessage {
                    timestamp: 1,
                    price: 1
                })
            )
        );

        let msg: RawMessage = [b'Q', 0, 0, 0, 1, 0, 0, 0, 1];
        assert_eq!(
            true,
            matches!(
                Message::from(msg),
                Message::Query(QueryMessage {
                    max_time: 1,
                    min_time: 1
                })
            )
        );

        let msg: RawMessage = [b'S', 0, 0, 0, 1, 0, 0, 0, 1];
        assert_eq!(
            false,
            matches!(
                Message::from(msg),
                Message::Insert(InsertMessage {
                    timestamp: 1,
                    price: 1
                })
            )
        );
    }
}

#[tokio::main]
async fn main() {
    set_var("RUST_LOG", "debug");
    env_logger::init();

    let port = std::env::args().nth(1).unwrap_or("9999".to_string());

    let bind_addr = format!("0.0.0.0:{}", port);

    let listener = TcpListener::bind(bind_addr.clone()).await.unwrap();

    log::info!("listening on {}", bind_addr);

    loop {
        let stream = match listener.accept().await {
            Ok((stream, _)) => stream,
            Err(error) => {
                log::error!("error accepting connection: {:?}", error);
                return;
            }
        };

        log::info!("accepted connection");

        tokio::spawn(async move {
            handle(stream).await;
        });
    }
}

async fn handle(stream: TcpStream) {
    let mut reader = tokio::io::BufReader::new(stream);

    let mut datastore: HashMap<i32, i32> = Default::default();

    let mut raw_msg = RawMessage::default();

    loop {
        match reader.read_exact(&mut raw_msg).await {
            Ok(_) => (),
            Err(_) => {
                return;
            }
        };

        let msg: Message = Message::from(raw_msg);

        match msg {
            Message::Insert(msg) => {
                datastore.insert(msg.timestamp, msg.price);
            }
            Message::Query(msg) => {
                log::debug!("amount of keys: {} query: {:?}", datastore.len(), msg);

                if msg.min_time > msg.max_time {
                    log::warn!(
                        "server requested min_time {} after max_time {}",
                        msg.min_time,
                        msg.max_time
                    );
                    reader.get_mut().write_all(&[0u8; 4]).await.unwrap();
                    continue;
                }

                let mut total_found = 0 as i64;
                let mut total = 0 as i64;

                datastore.iter().for_each(|(key, value)| {
                    if *key >= msg.min_time && *key <= msg.max_time {
                        total += *value as i64;
                        total_found += 1;
                    }
                });

                let mut mean_bytes: [u8; 4] = [0u8; 4];

                if total_found > 0 {
                    let mean = total / total_found;
                    byteorder::BigEndian::write_i32(&mut mean_bytes, mean as i32);
                    log::debug!("{} / {} = {} || {:?}", total, total_found, mean, mean_bytes);
                }

                reader.get_mut().write_all(&mean_bytes).await.unwrap();
            }
            _ => {
                log::debug!("undefined message");
                reader.get_mut().write_all(&[0u8; 4]).await.unwrap();
                continue;
            }
        }
    }
}
