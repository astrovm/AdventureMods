//! Tiny HTTP/1.1 server for unit tests that need specific headers or ranges.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

pub(crate) struct Reply {
    pub status: &'static str,
    pub headers: Vec<(&'static str, String)>,
    pub body: Vec<u8>,
}

impl Reply {
    pub fn ok(body: impl Into<Vec<u8>>) -> Self {
        Self {
            status: "200 OK",
            headers: Vec::new(),
            body: body.into(),
        }
    }

    pub fn status(mut self, status: &'static str) -> Self {
        self.status = status;
        self
    }

    pub fn header(mut self, name: &'static str, value: impl Into<String>) -> Self {
        self.headers.push((name, value.into()));
        self
    }
}

/// Parsed request line and headers, as the handler sees them.
pub(crate) struct Request {
    pub method: String,
    pub path: String,
    headers: Vec<(String, String)>,
}

impl Request {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

/// Serve `handler` on a random local port until the test process exits.
/// Returns the base URL and a log of the requests received.
pub(crate) fn serve(
    handler: impl Fn(&Request) -> Reply + Send + Sync + 'static,
) -> (String, Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let log = Arc::new(Mutex::new(Vec::new()));
    let requests = log.clone();

    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut buffer = [0u8; 8192];
            let size = stream.read(&mut buffer).unwrap_or(0);
            let text = String::from_utf8_lossy(&buffer[..size]).into_owned();
            let mut lines = text.lines();
            let mut first = lines.next().unwrap_or_default().split_whitespace();
            let request = Request {
                method: first.next().unwrap_or_default().to_owned(),
                path: first.next().unwrap_or_default().to_owned(),
                headers: lines
                    .take_while(|line| !line.is_empty())
                    .filter_map(|line| line.split_once(':'))
                    .map(|(key, value)| (key.trim().to_owned(), value.trim().to_owned()))
                    .collect(),
            };
            requests
                .lock()
                .unwrap()
                .push(format!("{} {}", request.method, request.path));

            let reply = handler(&request);
            let mut head = format!(
                "HTTP/1.1 {}\r\nContent-Length: {}\r\nConnection: close\r\n",
                reply.status,
                reply.body.len()
            );
            for (name, value) in &reply.headers {
                head.push_str(&format!("{name}: {value}\r\n"));
            }
            head.push_str("\r\n");
            let _ = stream.write_all(head.as_bytes());
            if request.method != "HEAD" {
                let _ = stream.write_all(&reply.body);
            }
        }
    });

    (base, log)
}
