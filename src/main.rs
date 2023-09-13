use std::{sync::mpsc, thread};

use chat::Connection;

fn main() {
    let mut c = Connection::new();
    c.on_receive(|s| {
        print!("Other used said \"{}\"", s);
    });
    let (tx, rx) = mpsc::channel::<String>();
    thread::spawn(move || loop {
        let mut input = String::new();
        let _ = std::io::stdin()
            .read_line(&mut input)
            .map(|_| tx.send(input))
            .map_err(|e| {
                eprintln!("Something happened on the input thread: {}", e);
                e
            });
    });
    loop {
        if let Ok(r) = rx.try_recv() {
            print!("Typed: {}", r);
            if c.is_connected() {
                println!("Trying to send...");
                let _ = c.send(&r).map_err(|e| {
                    eprintln!("Failure to send: {}", e);
                    e
                });
            } else {
                println!("Trying to connect to {}", r);
                let _ = c.connect(&r).map_err(|e| {
                    eprintln!("Failure to connect: {}", e);
                    e
                });
            }
        }
    }
}
