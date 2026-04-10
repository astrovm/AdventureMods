#![allow(dead_code)]

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub enum Response {
    Ok {
        content_type: &'static str,
        body: &'static str,
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
    thread: Option<std::thread::JoinHandle<()>>,
}

impl TestServer {
    pub fn start(routes: HashMap<&'static str, Response>) -> Self {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = listener.local_addr().unwrap();
        listener.set_nonblocking(true).unwrap();

        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_flag = shutdown.clone();
        let thread = std::thread::spawn(move || {
            while !shutdown_flag.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let mut buffer = [0u8; 4096];
                        let size = stream.read(&mut buffer).unwrap_or(0);
                        let request = String::from_utf8_lossy(&buffer[..size]);
                        let path = request
                            .lines()
                            .next()
                            .and_then(|line| line.split_whitespace().nth(1))
                            .unwrap_or("/");

                        let response = match routes.get(path) {
                            Some(Response::Ok { content_type, body }) => format!(
                                "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                                body.len(),
                                body
                            ),
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
                        };

                        let _ = stream.write_all(response.as_bytes());
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
            thread: Some(thread),
        }
    }

    pub fn url(&self, path: &str) -> String {
        format!("http://{}{}", self.address, path)
    }

    pub fn gamebanana_base(&self) -> String {
        self.url("/dl/")
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
