//! A tiny local HTTP/1.1 server for the download acceptance tests (`models.md`: "Use a local HTTP
//! test server serving a small generated file with known size and SHA-256"). Its behaviour can
//! be changed between requests to play every misbehaving server the spec describes.

use std::io::{BufRead, BufReader, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::Duration;

use sha2::{Digest, Sha256};

/// How the server answers the next requests.
#[derive(Debug, Clone)]
pub struct Behaviour {
    /// The bytes served.
    pub body: Vec<u8>,
    /// Answer a `Range` request with 206; when false, always send the whole file with 200.
    pub honour_range: bool,
    /// Added to the start of the range the server claims to send (a misbehaving server).
    pub range_start_offset: u64,
    /// Answer every range request with 416.
    pub refuse_range: bool,
    /// Announce this total size instead of the real one.
    pub advertised_size: Option<u64>,
    /// Send no `Content-Length` on a 200: the end of the body is the closed connection.
    pub omit_length: bool,
    /// Close the connection after sending this many body bytes ("stop the server").
    pub close_after: Option<usize>,
    /// Stop sending (keep the connection open) after this many body bytes.
    pub stall_after: Option<usize>,
    /// Body bytes per write, and the pause after each write (throttling).
    pub chunk: usize,
    pub chunk_delay: Duration,
}

impl Behaviour {
    pub fn serving(body: Vec<u8>) -> Self {
        Self {
            body,
            honour_range: true,
            range_start_offset: 0,
            refuse_range: false,
            advertised_size: None,
            omit_length: false,
            close_after: None,
            stall_after: None,
            chunk: 16 * 1024,
            chunk_delay: Duration::ZERO,
        }
    }
}

/// One request the server received.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Received {
    pub path: String,
    /// The value of the `Range` header, if any.
    pub range: Option<String>,
}

pub struct TestServer {
    port: u16,
    behaviour: Arc<Mutex<Behaviour>>,
    received: Arc<Mutex<Vec<Received>>>,
    stopping: Arc<AtomicBool>,
}

impl TestServer {
    pub fn start(behaviour: Behaviour) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = Self {
            port,
            behaviour: Arc::new(Mutex::new(behaviour)),
            received: Arc::default(),
            stopping: Arc::default(),
        };
        let behaviour = server.behaviour.clone();
        let received = server.received.clone();
        let stopping = server.stopping.clone();
        thread::spawn(move || {
            for stream in listener.incoming() {
                if stopping.load(Ordering::SeqCst) {
                    break;
                }
                let Ok(stream) = stream else { continue };
                let behaviour = behaviour.clone();
                let received = received.clone();
                let stopping = stopping.clone();
                thread::spawn(move || serve(stream, &behaviour, &received, &stopping));
            }
        });
        server
    }

    /// The URL of the served file.
    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}/model.gguf", self.port)
    }

    /// Changes how later requests are answered.
    pub fn behaviour(&self) -> MutexGuard<'_, Behaviour> {
        self.behaviour.lock().unwrap()
    }

    pub fn received(&self) -> Vec<Received> {
        self.received.lock().unwrap().clone()
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.stopping.store(true, Ordering::SeqCst);
        // Wake the accept loop so its thread ends.
        let _ = TcpStream::connect(("127.0.0.1", self.port));
    }
}

/// Deterministic pseudo-random bytes, so a wrong byte range shows up in the checksum.
pub fn test_bytes(len: usize) -> Vec<u8> {
    let mut state: u32 = 0x1234_5678;
    (0..len)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            (state >> 24) as u8
        })
        .collect()
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn serve(
    mut stream: TcpStream,
    behaviour: &Mutex<Behaviour>,
    received: &Mutex<Vec<Received>>,
    stopping: &AtomicBool,
) {
    let _ = stream.set_nodelay(true);
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).is_err() {
        return;
    }
    let path = request_line
        .split_whitespace()
        .nth(1)
        .unwrap_or_default()
        .to_owned();
    let mut range = None;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
            break;
        }
        if let Some((name, value)) = line.split_once(':')
            && name.trim().eq_ignore_ascii_case("range")
        {
            range = Some(value.trim().to_owned());
        }
    }
    if path.is_empty() {
        return;
    }
    received.lock().unwrap().push(Received {
        path,
        range: range.clone(),
    });
    let b = behaviour.lock().unwrap().clone();
    let total = b.body.len() as u64;
    let announced_total = b.advertised_size.unwrap_or(total);

    let requested_start = range
        .as_deref()
        .and_then(|r| r.strip_prefix("bytes="))
        .and_then(|r| r.split('-').next())
        .and_then(|s| s.parse::<u64>().ok());

    let (head, body): (String, &[u8]) = match requested_start {
        Some(_) if b.refuse_range => (
            format!(
                "HTTP/1.1 416 Range Not Satisfiable\r\nContent-Range: bytes */{total}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            ),
            &[],
        ),
        Some(start) if b.honour_range => {
            let claimed = start + b.range_start_offset;
            let from = (claimed.min(total)) as usize;
            let body = &b.body[from..];
            let end = total.saturating_sub(1);
            (
                format!(
                    "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {claimed}-{end}/{announced_total}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                ),
                body,
            )
        }
        _ => {
            let length = b.advertised_size.unwrap_or(total);
            let length = if b.omit_length {
                String::new()
            } else {
                format!("Content-Length: {length}\r\n")
            };
            (
                format!("HTTP/1.1 200 OK\r\n{length}Connection: close\r\n\r\n"),
                &b.body[..],
            )
        }
    };
    if stream.write_all(head.as_bytes()).is_err() {
        return;
    }
    let mut sent = 0usize;
    for piece in body.chunks(b.chunk.max(1)) {
        if let Some(limit) = b.stall_after
            && sent >= limit
        {
            // Hold the connection open without sending, until the test is over.
            for _ in 0..200 {
                if stopping.load(Ordering::SeqCst) {
                    break;
                }
                thread::sleep(Duration::from_millis(25));
            }
            break;
        }
        let piece = match b.close_after.or(b.stall_after) {
            Some(limit) if sent + piece.len() > limit => &piece[..limit - sent],
            _ => piece,
        };
        if stream.write_all(piece).is_err() {
            return;
        }
        let _ = stream.flush();
        sent += piece.len();
        if let Some(limit) = b.close_after
            && sent >= limit
        {
            break;
        }
        if !b.chunk_delay.is_zero() {
            thread::sleep(b.chunk_delay);
        }
    }
    let _ = stream.shutdown(Shutdown::Both);
}
