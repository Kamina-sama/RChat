use std::{
    error::Error,
    io::{Read, Write},
    net::{Ipv4Addr, SocketAddrV4, TcpListener, TcpStream},
    str::FromStr,
    sync::{Arc, RwLock},
    thread,
    time::Duration,
};

const PORT: u16 = 1024;

pub struct Connection {
    writer: Option<TcpStream>,
    received_messages_size: usize,
    received_messages: Arc<RwLock<String>>,
    on_receive: Option<Box<dyn FnMut(&str)>>,
}

impl Connection {
    pub fn new() -> Self {
        let ip = local_ip_address::local_ip().unwrap();
        println!("Local ip is {}", ip);

        let received_messages = Arc::new(RwLock::new(String::new()));
        let received_messages_clone = received_messages.clone();
        thread::spawn(move || {
            println!("Listener thread started...");
            let listener = TcpListener::bind("0.0.0.0:1024").unwrap();
            for incoming in listener.incoming() {
                if let Ok(mut stream) = incoming {
                    println!("Connection established");
                    stream
                        .set_read_timeout(Some(Duration::from_millis(50)))
                        .expect("Could not unwrap TcpStream...");
                    loop {
                        if let Ok(mut d) = received_messages_clone.write() {
                            match stream.read_to_string(&mut d) {
                                Ok(u) => {
                                    if u == 0 {
                                        break;
                                    }
                                }

                                Err(e) => println!("Error on TcpStream read: {}", e.to_string()),
                            }
                        }
                        thread::sleep(Duration::from_millis(50));
                    }
                    println!("Connection lost");
                }
            }
        });

        Connection {
            writer: None,
            on_receive: None,
            received_messages_size: 0,
            received_messages,
        }
    }
    pub fn on_receive<T: FnMut(&str) + 'static>(&mut self, t: T) {
        self.on_receive = Some(Box::new(t));
    }
    pub fn connect(&mut self, ip_address: &str) -> Result<(), Box<dyn Error>> {
        let ip = Ipv4Addr::from_str(ip_address)?;
        self.writer = Some(TcpStream::connect(SocketAddrV4::new(ip, PORT))?);
        Ok(())
    }
    pub fn is_connected(&self) -> bool {
        self.writer.is_some()
    }
    pub fn listen_loop(&mut self) -> Result<(), std::io::Error> {
        if let Ok(r) = self.received_messages.read() {
            r.lines().skip(self.received_messages_size).for_each(|x| {
                self.received_messages_size += 1;
                if let Some(on_receive) = self.on_receive.as_mut() {
                    on_receive(x);
                }
            });
        }
        Ok(())
    }
    pub fn send(&mut self, message: &str) -> Result<(), Box<dyn Error>> {
        if let Some(writer) = self.writer.as_mut() {
            writer.write_all(message.as_bytes())?;
            writer.flush()?;
        }
        Ok(())
    }
    pub fn disconnect(&mut self) -> Result<(), Box<dyn Error>> {
        if let Some(writer) = self.writer.as_mut() {
            writer.shutdown(std::net::Shutdown::Write)?;
        }
        self.writer = None;
        Ok(())
    }
}

pub fn run() {
    //c.on_receive(Callback).listen();
    //c.send("Hello");
    //c.disconnect();
}
