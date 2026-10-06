//! A tiny local HTTP server for the updater's feed tests: fixed routes, any
//! status code, and a log of the paths requested.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

#[derive(Clone)]
pub struct Response {
    pub status: u16,
    pub body: Vec<u8>,
}

impl Response {
    pub fn ok(body: impl Into<Vec<u8>>) -> Self {
        Self {
            status: 200,
            body: body.into(),
        }
    }

    pub fn status(status: u16) -> Self {
        Self {
            status,
            body: Vec::new(),
        }
    }
}

#[derive(Default)]
struct Shared {
    routes: HashMap<String, Response>,
    received: Vec<String>,
}

pub struct TestServer {
    port: u16,
    shared: Arc<Mutex<Shared>>,
    stopping: Arc<AtomicBool>,
}

impl TestServer {
    pub fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let shared = Arc::new(Mutex::new(Shared::default()));
        let stopping = Arc::new(AtomicBool::new(false));
        let (state, stop) = (shared.clone(), stopping.clone());
        thread::spawn(move || {
            for stream in listener.incoming() {
                if stop.load(Ordering::SeqCst) {
                    break;
                }
                if let Ok(stream) = stream {
                    let state = state.clone();
                    thread::spawn(move || serve(stream, &state));
                }
            }
        });
        Self {
            port,
            shared,
            stopping,
        }
    }

    /// The URL of `path` (which starts with `/`).
    pub fn url(&self, path: &str) -> String {
        format!("http://127.0.0.1:{}{path}", self.port)
    }

    pub fn route(&self, path: &str, response: Response) {
        let mut shared = self.shared.lock().unwrap();
        shared.routes.insert(path.to_owned(), response);
    }

    /// The paths requested so far, in order.
    pub fn received(&self) -> Vec<String> {
        self.shared.lock().unwrap().received.clone()
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.stopping.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(("127.0.0.1", self.port));
    }
}

fn serve(stream: TcpStream, shared: &Mutex<Shared>) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).is_err() || request_line.is_empty() {
        return;
    }
    loop {
        let mut header = String::new();
        match reader.read_line(&mut header) {
            Ok(0) | Err(_) => break,
            Ok(_) if header.trim().is_empty() => break,
            Ok(_) => {}
        }
    }
    let path = request_line
        .split_whitespace()
        .nth(1)
        .unwrap_or("/")
        .to_owned();
    let response = {
        let mut shared = shared.lock().unwrap();
        shared.received.push(path.clone());
        shared
            .routes
            .get(&path)
            .cloned()
            .unwrap_or(Response::status(404))
    };
    let mut stream = stream;
    let head = format!(
        "HTTP/1.1 {} X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        response.status,
        response.body.len()
    );
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(&response.body);
    let _ = stream.flush();
}
