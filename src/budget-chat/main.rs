use std::collections::HashMap;
use std::collections::HashSet;
use std::env::set_var;
use std::sync::Arc;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::sync::Mutex;

#[derive(Debug)]
enum Error {
    InvalidNick,
    InvalidMessage,
}

#[derive(Debug, Default, Clone)]
struct RoomMessage {
    user: String,
    message: String,
}

#[derive(Debug, Default)]
struct State {
    nick: String,
    users: HashSet<String>,
    handles: HashMap<String, mpsc::Sender<RoomMessage>>,
}

impl State {
    fn has_user(&self, nick: String) -> bool {
        self.users.contains(&nick)
    }

    fn new_handle(&mut self, nick: String) -> mpsc::Receiver<RoomMessage> {
        let (sender, receiver) = mpsc::channel::<RoomMessage>(1);
        self.handles.insert(nick, sender);
        receiver
    }

    fn delete_user(&mut self, nick: String) {
        self.users.remove(&nick);
        self.handles.remove(&nick);
    }

    async fn send_message(&self, message: &RoomMessage) {
        for handle in self.handles.clone().into_iter() {
            if handle.0 == message.user {
                continue;
            }

            handle.1.send(message.clone()).await.unwrap();
        }
    }
}

type SafeState = Arc<Mutex<State>>;

#[tokio::main]
async fn main() {
    set_var("RUST_LOG", "debug");
    env_logger::init();

    let port = std::env::args().nth(1).unwrap_or("9999".to_string());

    let bind_addr = format!("0.0.0.0:{}", port);

    let listener = TcpListener::bind(bind_addr.clone()).await.unwrap();

    let state = Arc::new(Mutex::new(State::default()));

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

        let state = state.clone();
        tokio::spawn(async move {
            handle(stream, state).await;
        });
    }
}

async fn handle(stream: TcpStream, state: SafeState) {
    let mut reader = tokio::io::BufReader::new(stream);

    let mut maybe_recv: Option<mpsc::Receiver<RoomMessage>> = None;
    let mut nick = String::new();

    loop {
        if maybe_recv.is_none() {
            let new_recv = request_username(&mut reader, &state).await.unwrap();
            maybe_recv = Some(new_recv.0);

            nick = new_recv.1.trim().to_string();

            let names: Vec<String> = state.lock().await.handles.keys().cloned().collect();

            reader
                .get_mut()
                .write_all(format!("* The room contains: {}\n", names.join(", ")).as_bytes())
                .await
                .unwrap();
            continue;
        }

        let recv = maybe_recv.as_mut().unwrap();

        log::debug!("{}: checking other user's messages", nick);
        match recv.try_recv() {
            Ok(msg) => {
                reader
                    .get_mut()
                    .write_all(format!("[{}]: {}", msg.user, msg.message).as_bytes())
                    .await
                    .unwrap();
            }
            Err(_) => (),
        };

        log::debug!("{}: done", nick);

        log::debug!("{}: checking message to send", nick);
        let mut sent_message = String::new();
        
        if !reader.fill_buf().await.unwrap_or_default().is_empty() {
            match reader.read_line(&mut sent_message).await {
                Ok(_) => {
                    state
                        .lock()
                        .await
                        .send_message(&RoomMessage {
                            user: nick.clone(),
                            message: sent_message.clone(),
                        })
                        .await;
                }
                Err(_) => (),
            };
        }
        log::debug!("{}: done", nick);
    }
}

async fn request_username(
    reader: &mut BufReader<TcpStream>,
    state: &SafeState,
) -> Result<(mpsc::Receiver<RoomMessage>, String), Error> {
    reader
        .get_mut()
        .write_all("Nick?\n".as_bytes())
        .await
        .unwrap();

    let mut maybe_nick = String::new();

    reader.read_line(&mut maybe_nick).await.unwrap();

    if !valid_nick(&maybe_nick) {
        return Err(Error::InvalidNick);
    }

    maybe_nick = maybe_nick.trim().to_string();

    let nick_clone = maybe_nick.clone();

    Ok((state.lock().await.new_handle(maybe_nick), nick_clone))
}

fn valid_nick(nick: &String) -> bool {
    match nick.len() {
        0..=128 => true,
        _ => false,
    }
}

fn valid_message(message: String) -> bool {
    match message.len() {
        0..=1000 => true,
        _ => false,
    }
}
