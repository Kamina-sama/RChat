use rand::{distributions::Alphanumeric, Rng};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    error::Error,
    io::{self, Read, Write},
    net::{Ipv4Addr, SocketAddrV4, TcpListener, TcpStream},
    str::FromStr,
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

const PORT: u16 = 1024;

fn get_self_socket() -> SocketAddrV4 {
    let ip = local_ip_address::local_ip().unwrap();
    match ip {
        std::net::IpAddr::V4(i) => SocketAddrV4::new(i, PORT),
        std::net::IpAddr::V6(_) => panic!("fuck you"),
    }
}

pub struct Chat2 {
    messages: Vec<Message>,
    client: ChatClient,
    client_rx: Receiver<String>,
    input_rx: Receiver<String>,
}

impl Chat2 {
    pub fn new() -> Self {
        let ip = local_ip_address::local_ip();
        println!("{}", ip.map_or_else(|a| a.to_string(), |b| b.to_string()));

        let (client_tx, client_rx) = mpsc::channel();
        let (input_tx, input_rx) = mpsc::channel();

        thread::spawn(move || loop {
            let mut input = String::new();
            match io::stdin().read_line(&mut input) {
                Ok(_) => input_tx.send(input).unwrap_or_else(|e| {
                    eprintln!("Could not send input: {}", e);
                }),
                Err(e) => eprintln!("Could not read input: {}", e),
            }
        });
        ChatServer::start_listening(client_tx);
        Chat2 {
            messages: Vec::new(),
            client_rx,
            input_rx,
            client: ChatClient::new(),
        }
    }
    pub fn looop(&mut self) {
        loop {
            self.client_rx.try_iter().for_each(|m| {
                let m: Message = ron::from_str(&m).unwrap();
                match m {
                    Message::Connected(socket) => {
                        self.client.receivers.insert(socket);
                    }
                    Message::Disconnect(socket) => {
                        self.client.receivers.remove(&socket);
                    }
                    Message::GetReceivers(socket) => {
                        self.client.broadcast_message(Message::PostReceivers(
                            self.client.receivers.clone(),
                        ));
                    }
                    Message::PostReceivers(r) => {
                        self.client.receivers.extend(r.iter());
                        self.client.broadcast_message(Message::Connected(get_self_socket()));
                    }
                    _ => {
                        self.messages.push(m);
                        println!("Messages is now: {:?}", self.messages);
                    }
                }
            });
            self.input_rx.try_iter().for_each(|i| {
                if i.starts_with("!connect") {
                    i.split(' ')
                        .skip(1)
                        .next()
                        .and_then(|ip| self.client.add_receiver(ip).err())
                        .and_then(|e| {
                            eprintln!("Wrong parsing of ip: {}", e);
                            Some(())
                        });
                    println!("List of receivers is now {:?}", self.client.receivers);
                } else if i.starts_with("!disconnect") {
                    self.client
                        .broadcast_message(Message::Disconnect(get_self_socket()));
                    self.client.clear_receivers();
                    println!("List of receivers is now {:?}", self.client.receivers);
                } else if i.starts_with("!setname") {
                    i.split(' ').skip(1).next().and_then(|name| {
                        self.client.set_user_name(name);
                        Some(())
                    });
                    println!("Name is now {}", self.client.user_name);
                } else {
                    self.client.broadcast_normal_message(i).unwrap_or_else(|e| {
                        eprintln!("Could not send: {}", e);
                    });
                }
            });
        }
    }
}

struct ChatServer;

impl ChatServer {
    fn start_listening(tx: Sender<String>) {
        thread::spawn(move || {
            println!("Listener thread started...");
            let listener = TcpListener::bind("0.0.0.0:1024").unwrap();
            for incoming in listener.incoming() {
                if let Ok(mut stream) = incoming {
                    println!("Connection established");
                    let mut received = String::new();
                    if let Ok(_) = stream.read_to_string(&mut received) {
                        tx.send(received).unwrap_or_else(|e| {
                            eprintln!("Could not send received string: {}", e);
                        });
                    }
                    println!("Connection lost");
                }
            }
        });
    }
}

#[derive(Serialize, Deserialize, Debug)]
enum Message {
    Normal {
        author: String,
        content: String,
        time_stamp: u64,
    },
    Connected(SocketAddrV4),
    GetReceivers(SocketAddrV4),
    PostReceivers(HashSet<SocketAddrV4>),
    Disconnect(SocketAddrV4),
}

struct ChatClient {
    user_name: String,
    receivers: HashSet<SocketAddrV4>,
}

impl ChatClient {
    fn new() -> Self {
        let user_name = format!(
            "user {}",
            rand::thread_rng()
                .sample_iter(&Alphanumeric)
                .take(6)
                .map(char::from)
                .collect::<String>()
        );
        ChatClient {
            user_name,
            receivers: HashSet::new(),
        }
    }
    fn unicast_message(
        &self,
        message: Message,
        socket: SocketAddrV4,
    ) -> Result<(), Box<dyn Error>> {
        self.unicast_raw(ron::to_string(&message)?, socket)?;
        Ok(())
    }
    fn broadcast_message(&self, message: Message) -> Result<(), Box<dyn Error>> {
        self.broadcast_raw(ron::to_string(&message)?)?;
        Ok(())
    }
    fn broadcast_normal_message(&self, message: String) -> Result<(), Box<dyn Error>> {
        let now = SystemTime::now();
        let time_stamp = now.duration_since(UNIX_EPOCH).unwrap().as_secs();
        let message = Message::Normal {
            time_stamp,
            content: message,
            author: self.user_name.clone(),
        };
        self.broadcast_raw(ron::to_string(&message)?)?;
        Ok(())
    }
    fn broadcast_raw(&self, message: String) -> Result<(), Box<dyn Error>> {
        for receiver in &self.receivers {
            let mut stream = TcpStream::connect(receiver)?;
            stream.write_all(message.as_bytes())?;
            stream.flush()?;
            stream.shutdown(std::net::Shutdown::Write)?;
        }
        Ok(())
    }
    fn unicast_raw(&self, message: String, socket: SocketAddrV4) -> Result<(), Box<dyn Error>> {
        if let Some(s) = self.receivers.get(&socket) {
            let mut stream = TcpStream::connect(s)?;
            stream.write_all(message.as_bytes())?;
            stream.flush()?;
            stream.shutdown(std::net::Shutdown::Write)?;
        }
        Ok(())
    }
    fn set_user_name(&mut self, user_name: &str) {
        self.user_name = String::from(user_name.trim());
    }
    fn add_receiver(&mut self, ip: &str) -> Result<(), Box<dyn Error>> {
        let socket = SocketAddrV4::new(Ipv4Addr::from_str(ip.trim())?, PORT);
        self.receivers.insert(socket.clone());
        self.unicast_message(Message::GetReceivers(get_self_socket()), socket)?;
        Ok(())
    }
    fn clear_receivers(&mut self) {
        self.receivers.clear();
    }
}
