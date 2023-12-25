use std::collections::HashMap;
use std::collections::HashSet;
use std::env::set_var;
use std::sync::Arc;
use std::time::Duration;
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
    let mut initialized = false;
    let mut nick = String::new();

    let (socket_tx, mut socket_rx) = protohackers::socket::socket_rxtx(stream);

    let mut recv = None;

    loop {
        if !initialized {
            let new_recv = request_username(&socket_tx, &mut socket_rx, &state)
                .await
                .unwrap();

            nick = new_recv.1.trim().to_string();

            let names: Vec<String> = state.lock().await.handles.keys().cloned().collect();

            socket_tx
                .send(format!("* The room contains: {}\n", names.join(", ")))
                .await
                .unwrap();

            initialized = true;
            recv = Some(new_recv.0);

            continue;
        }

        let recv = recv.as_mut().unwrap();

        tokio::select! {
            user_input = socket_rx.recv() => {
                let user_input = user_input.unwrap();
                if let Some(user_input) = user_input {
                    state
                        .lock()
                        .await
                        .send_message(&RoomMessage {
                            user: nick.clone(),
                            message: user_input.clone(),
                        })
                        .await;
                }
            }
            other_user_msg = recv.recv() => {
                if let Some(other_user_msg) = other_user_msg {
                    let fmtd_msg = format!("[{}]: {}", other_user_msg.user, other_user_msg.message);
                    socket_tx.send(fmtd_msg).await.unwrap();
                }

            }
        }
    }
}

async fn request_username(
    socket_tx: &mpsc::Sender<String>,
    socket_rx: &mut mpsc::Receiver<Option<String>>,
    state: &SafeState,
) -> Result<(mpsc::Receiver<RoomMessage>, String), Error> {
    socket_tx.send("Nick?\n".to_string()).await.unwrap();

    let maybe_nick = {
        loop {
            if let Some(s) = socket_rx.recv().await.unwrap() {
                break s;
            }
        }
    };

    if !valid_nick(&maybe_nick) {
        return Err(Error::InvalidNick);
    }

    let maybe_nick = maybe_nick.trim().to_string();

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
