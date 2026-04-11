#![allow(dead_code)]

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};

pub enum Response {
    Ok {
        content_type: &'static str,
        body: &'static str,
    },
    SlowOk {
        content_type: &'static str,
        body: &'static str,
        delay_ms: u64,
    },
    Redirect(&'static str),
    Status {
        status_line: &'static str,
        content_type: &'static str,
        body: &'static str,
    },
}

pub struct TestServer {
    address: SocketAddr,
    shutdown: Arc<AtomicBool>,
    max_active_requests: Arc<AtomicUsize>,
    thread: Option<std::thread::JoinHandle<()>>,
}

/// Maps (item_type, item_id) -> file_id for the fake GameBanana API.
///
/// In production, item_id and file_id are distinct: an item (e.g. a mod page) has
/// one or more file entries whose IDs differ from the item ID. Using the same value
/// for both in tests would hide bugs in the resolution path.
///
/// The file_id is item_id * 10 + 1 so the difference is always detectable.
fn fake_file_id(item_id: u64) -> u64 {
    item_id * 10 + 1
}

/// Parse `itemid=N` from a query string, returning N as a string.
fn parse_itemid(query: &str) -> Option<String> {
    query.split('&').find_map(|part| {
        let (k, v) = part.split_once('=')?;
        if k == "itemid" {
            Some(v.to_string())
        } else {
            None
        }
    })
}

/// Parse `itemtype=X` from a query string.
fn parse_itemtype(query: &str) -> Option<String> {
    query.split('&').find_map(|part| {
        let (k, v) = part.split_once('=')?;
        if k == "itemtype" {
            Some(v.to_string())
        } else {
            None
        }
    })
}

/// Build a fake GameBanana API response where the file key differs from item_id.
///
/// Format: `[{"<file_id>": {"_idRow": <file_id>}}]`
/// The `file_id` is deliberately distinct from `item_id` so tests exercise the
/// item->file resolution logic rather than passing trivially with item_id==file_id.
fn fake_gamebanana_api_response(item_id: &str) -> String {
    let id: u64 = item_id.parse().unwrap_or(0);
    let file_id = fake_file_id(id);
    format!("[{{\"{}\":{{\"_idRow\":{}}}}}]", file_id, file_id)
}

impl TestServer {
    pub fn start(routes: HashMap<&'static str, Response>) -> Self {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = listener.local_addr().unwrap();
        listener.set_nonblocking(true).unwrap();

        let shutdown = Arc::new(AtomicBool::new(false));
        let routes = Arc::new(routes);
        let active_requests = Arc::new(AtomicUsize::new(0));
        let max_active_requests = Arc::new(AtomicUsize::new(0));
        let shutdown_flag = shutdown.clone();
        let active_requests_for_thread = active_requests.clone();
        let max_active_requests_for_thread = max_active_requests.clone();
        let thread = std::thread::spawn(move || {
            while !shutdown_flag.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let routes = routes.clone();
                        let active_requests = active_requests_for_thread.clone();
                        let max_active_requests = max_active_requests_for_thread.clone();
                        std::thread::spawn(move || {
                            let current = active_requests.fetch_add(1, AtomicOrdering::SeqCst) + 1;
                            max_active_requests.fetch_max(current, AtomicOrdering::SeqCst);

                            let mut buffer = [0u8; 4096];
                            let size = stream.read(&mut buffer).unwrap_or(0);
                            let request = String::from_utf8_lossy(&buffer[..size]);
                            let full_path = request
                                .lines()
                                .next()
                                .and_then(|line| line.split_whitespace().nth(1))
                                .unwrap_or("/");

                            let (path, query) =
                                full_path.split_once('?').unwrap_or((full_path, ""));

                            let response = if path == "/gbapi" {
                                match (parse_itemtype(query), parse_itemid(query)) {
                                    (Some(_item_type), Some(id)) => {
                                        let body = fake_gamebanana_api_response(&id);
                                        format!(
                                            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                                            body.len(),
                                            body
                                        )
                                    }
                                    _ => "HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_string(),
                                }
                            } else {
                                match routes.get(path) {
                                    Some(Response::Ok { content_type, body }) => format!(
                                        "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                                        body.len(),
                                        body
                                    ),
                                    Some(Response::SlowOk {
                                        content_type,
                                        body,
                                        delay_ms,
                                    }) => {
                                        std::thread::sleep(std::time::Duration::from_millis(*delay_ms));
                                        format!(
                                            "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                                            body.len(),
                                            body
                                        )
                                    }
                                    Some(Response::Redirect(location)) => format!(
                                        "HTTP/1.1 302 Found\r\nLocation: http://{}{}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                                        address, location
                                    ),
                                    Some(Response::Status {
                                        status_line,
                                        content_type,
                                        body,
                                    }) => format!(
                                        "HTTP/1.1 {status_line}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                                        body.len(),
                                        body
                                    ),
                                    None => "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_string(),
                                }
                            };

                            let _ = stream.write_all(response.as_bytes());
                            active_requests.fetch_sub(1, AtomicOrdering::SeqCst);
                        });
                    }
                    Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(std::time::Duration::from_millis(10));
                    }
                    Err(_) => break,
                }
            }
        });

        Self {
            address,
            shutdown,
            max_active_requests,
            thread: Some(thread),
        }
    }

    pub fn url(&self, path: &str) -> String {
        format!("http://{}{}", self.address, path)
    }

    /// Base URL for the fake GameBanana API endpoint (sets ADVENTURE_MODS_GAMEBANANA_API_BASE).
    pub fn gamebanana_api_base(&self) -> String {
        self.url("/gbapi?fields=Files().aFiles()")
    }

    /// Base URL for GameBanana downloads (sets ADVENTURE_MODS_GAMEBANANA_DL_BASE).
    pub fn gamebanana_dl_base(&self) -> String {
        self.url("/dl/")
    }

    /// Returns the fake file_id the test server resolves for a given item_id.
    ///
    /// Use this in tests to register the correct `/dl/<file_id>` route instead of
    /// accidentally using the item_id as the download path.
    pub fn fake_file_id_for(item_id: u32) -> u32 {
        fake_file_id(item_id as u64) as u32
    }

    pub fn max_active_requests(&self) -> usize {
        self.max_active_requests.load(AtomicOrdering::SeqCst)
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        let _ = std::net::TcpStream::connect(self.address);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
