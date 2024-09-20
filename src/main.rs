use events::{Event, EventQueue};
use std::{
    net::{TcpListener, TcpStream},
    thread,
    fs::File,
    io::{BufReader, BufRead, Read},
    sync::{
        Arc,
        atomic::{
            Ordering,
            AtomicUsize,
        },
    },
};
use http::HTTPHeader;
use config::Config;

pub mod events;
pub mod http;
pub mod config;

fn handle_event(_ev: Event) {}

fn handle_connection(stream: TcpStream, mut eq: EventQueue) {
    eq.dispatch(Event::Connection { client_addr: stream.peer_addr().expect("Could not determine peer address") })
        .expect("Could not dispatch connection event");
    // wait for incoming messages, parse into HTTPRequest struct,
    let mut keep_alive = true;

    while keep_alive {
        let mut reader = BufReader::new(&stream);
        let mut request_line = String::new();

        // Read the request line (e.g., "GET / HTTP/1.1")
        reader.read_line(&mut request_line).unwrap();

        if request_line.is_empty() {
            break; // End of stream, close connection.
        }

        let request_line_parts = request_line.split(" ").collect::<Vec<_>>();
        let path = request_line_parts[1].to_string();

        // Prepare to store the headers and body
        let mut headers = Vec::new();
        let mut body = Vec::new();

        // Read headers
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();

            // If we encounter a blank line, headers are done
            if line == "\r\n" {
                break;
            }

            // Parse each header line as "name: value"
            if let Some(colon_pos) = line.find(':') {
                let name = line[..colon_pos].trim().to_string();
                let value = line[colon_pos + 1..].trim().to_string();

                // Check if the connection is persistent
                if name.eq_ignore_ascii_case("Connection") && value.eq_ignore_ascii_case("close") {
                    keep_alive = false; // Client requested to close the connection.
                }

                headers.push(HTTPHeader { name, value });
            }
        }

        // Parse Content-Length to read the body if present
        if let Some(content_length_header) = headers
            .iter()
            .find(|h| h.name.eq_ignore_ascii_case("Content-Length"))
        {
            let content_length = content_length_header.value.parse::<usize>().unwrap_or(0);
            body.resize(content_length, 0);
            reader.read_exact(&mut body).unwrap();
        }

        eq.dispatch(Event::HTTPRequest { path , headers, body }).expect("Could not dispatch event");

        // If keep_alive is false, break out of the loop to close the connection
        if !keep_alive {
            break;
        }
    }
}

fn event_loop(eq: EventQueue) {
    loop {
        match eq.retrieve() {
            Ok(ev) => {
                handle_event(ev);
            }
            Err(e) => {
                println!("Error: {e:}");
            }
        }
    }
}


pub struct FallbackChain {
    fallbacks: Vec<&'static str>,
}

// intended use:
// for file in fallback_chain {
//    // do something useful with 'file'
// }
// no need to 'break' from the for loop, the iterator will stop when
// a file is successfully opened
impl Iterator for FallbackChain {
    type Item = File;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(path) = self.fallbacks.pop() {
            match File::open(path) {
                Ok(f) => {
                    // if we have a success, we can stop iterating
                    self.fallbacks.clear();
                    Some(f)
                }
                Err(_) => {
                    None
                }
            }
        } else {
            None
        }
    }
}

impl FallbackChain {
    fn new(fallbacks: Vec<&'static str>) -> Self {
        Self {
            fallbacks,
        }
    }
}


fn main() {
    let config_path_fallback_chain = FallbackChain::new(vec![
        "/etc/fritz/fritz.conf",
        "/etc/fritz.conf",
    ]);
    let config = Config::read(config_path_fallback_chain);
    let listener = TcpListener::bind("127.0.0.1:80").expect("Could not bind to port 80");
    let eq = EventQueue::new();
    let eq_for_event_loop = eq.clone();
    let active_connections = Arc::new(AtomicUsize::new(0));
    thread::spawn(move || {
        event_loop(eq_for_event_loop);
    });
    for stream in listener.incoming() {
        let active_connections = active_connections.clone();
        if active_connections.load(Ordering::SeqCst) < config.max_connections {
            active_connections.fetch_add(1, Ordering::SeqCst);
            match stream {
                Ok(stream) => {
                    println!("{stream:?}");
                    let eq = eq.clone();
                    thread::spawn(move || {
                        handle_connection(stream, eq);
                        active_connections.fetch_sub(1, Ordering::SeqCst);
                    });
                }
                Err(e) => {
                    println!("Couldn't accept client: {e:?}");
                }
            }
        }
    }
}
