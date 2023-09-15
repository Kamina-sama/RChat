use rand::{distributions::Alphanumeric, Rng};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    error::Error,
    io::{self, Read, Write},
    net::{Ipv4Addr, SocketAddrV4, TcpListener, TcpStream},
    str::FromStr,
    sync::mpsc::{self, Receiver, Sender},
    thread::{self, JoinHandle},
    time::{SystemTime, UNIX_EPOCH},
};

const PORT: u16 = 1024;

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
                    Message::Disconnect(socket) => {
                        self.client.receivers.remove(&socket);
                    }
                    Message::GetReceivers => {
                        self.client
                            .send_message(Message::PostReceivers(self.client.receivers.clone()));
                    }
                    Message::PostReceivers(r) => {
                        self.client.receivers.extend(r.iter());
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
                    let ip = local_ip_address::local_ip().unwrap();
                    let ip = match ip {
                        std::net::IpAddr::V4(i) => i,
                        std::net::IpAddr::V6(_) => panic!("fuck you"),
                    };
                    self.client
                        .send_message(Message::Disconnect(SocketAddrV4::new(ip, PORT)));
                    self.client.clear_receivers();
                    println!("List of receivers is now {:?}", self.client.receivers);
                } else if i.starts_with("!setname") {
                    i.split(' ').skip(1).next().and_then(|name| {
                        self.client.set_user_name(name);
                        Some(())
                    });
                    println!("Name is now {}", self.client.user_name);
                } else {
                    self.client.send_normal_message(i).unwrap_or_else(|e| {
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
    GetReceivers,
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
    fn send_message(&self, message: Message) -> Result<(), Box<dyn Error>> {
        self.send_raw(ron::to_string(&message)?)?;
        Ok(())
    }
    fn send_normal_message(&self, message: String) -> Result<(), Box<dyn Error>> {
        let now = SystemTime::now();
        let time_stamp = now.duration_since(UNIX_EPOCH).unwrap().as_secs();
        let message = Message::Normal {
            time_stamp,
            content: message,
            author: self.user_name.clone(),
        };
        self.send_raw(ron::to_string(&message)?)?;
        Ok(())
    }
    fn send_raw(&self, message: String) -> Result<(), Box<dyn Error>> {
        for receiver in &self.receivers {
            let mut stream = TcpStream::connect(receiver)?;
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
        self.receivers
            .insert(SocketAddrV4::new(Ipv4Addr::from_str(ip.trim())?, PORT));
        self.send_message(Message::GetReceivers)?;
        Ok(())
    }
    fn clear_receivers(&mut self) {
        self.receivers.clear();
    }
}
