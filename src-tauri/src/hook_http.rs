//! Authenticated, bounded, loopback-only HTTP for the permission bridge.
use serde_json::{json, Value};
use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc, Mutex,
};
use std::thread;
use std::time::{Duration, Instant};

pub const MAX_BODY: usize = 64 * 1024;
const MAX_HEADER: usize = 8192;
const READ_BUDGET: Duration = Duration::from_secs(2);

fn token_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |diff, (a, b)| diff | (a ^ b)) == 0
}
pub fn parse<R: Read>(reader: &mut R, token: &str, port: u16) -> Result<Value, u16> {
    let mut header = Vec::new();
    while !header.ends_with(b"\r\n\r\n") {
        if header.len() >= MAX_HEADER {
            return Err(431);
        }
        let mut byte = [0u8];
        reader.read_exact(&mut byte).map_err(|_| 408u16)?;
        header.push(byte[0]);
    }
    let text = std::str::from_utf8(&header).map_err(|_| 400u16)?;
    let mut lines = text.split("\r\n");
    let request: Vec<_> = lines.next().unwrap_or("").split_whitespace().collect();
    if request.len() != 3 || !matches!(request[2], "HTTP/1.1" | "HTTP/1.0") {
        return Err(400);
    }
    if request[1] != "/permission" {
        return Err(404);
    }
    if request[0] != "POST" {
        return Err(405);
    }
    let mut headers = std::collections::HashMap::new();
    for line in lines.filter(|l| !l.is_empty()) {
        let (name, value) = line.split_once(':').ok_or(400u16)?;
        if name.is_empty()
            || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
            || value.contains(['\r', '\n'])
        {
            return Err(400);
        }
        if headers
            .insert(name.to_ascii_lowercase(), value.trim())
            .is_some()
        {
            return Err(400);
        }
    }
    // Browsers are not hook clients. Reject before reading a body or emitting UI.
    if headers.contains_key("origin") || headers.contains_key("transfer-encoding") {
        return Err(403);
    }
    let expected = format!("Bearer {token}");
    if !token_eq(
        headers.get("authorization").unwrap_or(&"").as_bytes(),
        expected.as_bytes(),
    ) {
        return Err(403);
    }
    let expected_host = format!("127.0.0.1:{port}");
    let localhost = format!("localhost:{port}");
    if !matches!(headers.get("host"), Some(h) if *h == expected_host || *h == localhost) {
        return Err(403);
    }
    if headers
        .get("content-type")
        .and_then(|s| s.split(';').next())
        .map(|s| s.trim().eq_ignore_ascii_case("application/json"))
        != Some(true)
    {
        return Err(415);
    }
    let length: usize = headers
        .get("content-length")
        .ok_or(411u16)?
        .parse()
        .map_err(|_| 400u16)?;
    if length > MAX_BODY {
        return Err(413);
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body).map_err(|_| 408u16)?;
    let value: Value = serde_json::from_slice(&body).map_err(|_| 400u16)?;
    if value["hook_event_name"] != "PermissionRequest"
        || !value["tool_name"]
            .as_str()
            .is_some_and(|s| !s.is_empty() && s.len() <= 256)
        || !value["tool_input"].is_object()
        || value
            .get("cwd")
            .is_some_and(|v| !v.is_null() && !v.as_str().is_some_and(|s| s.len() <= 4096))
    {
        return Err(400);
    }
    Ok(value)
}
struct Deadline<'a> {
    stream: &'a mut TcpStream,
    until: Instant,
}
impl Read for Deadline<'_> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        let left = self
            .until
            .checked_duration_since(Instant::now())
            .filter(|d| !d.is_zero())
            .ok_or(io::ErrorKind::TimedOut)?;
        self.stream.set_read_timeout(Some(left))?;
        self.stream.read(bytes)
    }
}
fn respond(stream: &mut TcpStream, status: u16, body: Value) {
    let body = body.to_string();
    let reason = if status == 200 {
        "OK"
    } else {
        "Request Rejected"
    };
    let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
    let _ = write!(stream,"HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{body}",body.len());
}

pub struct Service {
    pub address: SocketAddr,
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}
impl Service {
    pub fn start(
        token: String,
        handler: impl Fn(Value) -> Result<Value, u16> + Send + Sync + 'static,
    ) -> io::Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        let address = listener.local_addr()?;
        listener.set_nonblocking(true)?;
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let token = Arc::new(token);
        let handler = Arc::new(handler);
        let (tx, rx) = mpsc::sync_channel::<(TcpStream, Instant)>(8);
        let rx = Arc::new(Mutex::new(rx));
        for _ in 0..8 {
            let (rx, token, handler) = (rx.clone(), token.clone(), handler.clone());
            thread::spawn(move || loop {
                let Ok((mut stream, received)) = rx.lock().unwrap().recv() else {
                    break;
                };
                let mut reader = Deadline {
                    stream: &mut stream,
                    until: received + READ_BUDGET,
                };
                let result =
                    parse(&mut reader, &token, address.port()).and_then(|body| handler(body));
                match result {
                    Ok(body) => respond(&mut stream, 200, body),
                    Err(status) => {
                        respond(&mut stream, status, json!({"error":"Request rejected"}))
                    }
                }
            });
        }
        let thread = thread::spawn(move || {
            while !stopped.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, peer)) => {
                        // Windows can inherit the listener's nonblocking mode.
                        // Worker reads need deadline timeouts, not WouldBlock.
                        if stream.set_nonblocking(false).is_err() {
                            continue;
                        }
                        if !peer.ip().is_loopback() {
                            respond(&mut stream, 403, json!({}));
                            continue;
                        }
                        if let Err(error) = tx.try_send((stream, Instant::now())) {
                            let (mut stream, _) = match error {
                                mpsc::TrySendError::Full(v)
                                | mpsc::TrySendError::Disconnected(v) => v,
                            };
                            respond(&mut stream, 503, json!({"error":"Busy"}));
                        }
                    }
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(8))
                    }
                    Err(_) => break,
                }
            }
        });
        Ok(Self {
            address,
            stop,
            thread: Some(thread),
        })
    }
}
impl Drop for Service {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const BODY: &str = r#"{"hook_event_name":"PermissionRequest","tool_name":"Bash","tool_input":{"command":"echo test"}}"#;
    fn raw(extra: &str) -> Vec<u8> {
        format!("POST /permission HTTP/1.1\r\nHost: 127.0.0.1:1234\r\nAuthorization: Bearer fixture-token\r\nContent-Type: application/json\r\nContent-Length: {}\r\n{extra}\r\n{BODY}",BODY.len()).into_bytes()
    }
    #[test]
    fn valid_hook_is_accepted() {
        assert!(parse(&mut raw("").as_slice(), "fixture-token", 1234).is_ok());
    }
    #[test]
    fn auth_and_browser_origin_are_checked_before_body() {
        assert_eq!(
            parse(&mut raw("").as_slice(), "wrong-token", 1234).unwrap_err(),
            403
        );
        assert_eq!(
            parse(
                &mut raw("Origin: https://example.invalid\r\n").as_slice(),
                "fixture-token",
                1234
            )
            .unwrap_err(),
            403
        );
    }
    #[test]
    fn method_path_and_host_are_restricted() {
        for (from, to, code) in [
            ("POST ", "GET ", 405),
            ("/permission ", "/other ", 404),
            ("127.0.0.1:1234", "evil.invalid", 403),
        ] {
            let input = String::from_utf8(raw("")).unwrap().replacen(from, to, 1);
            assert_eq!(
                parse(&mut input.as_bytes(), "fixture-token", 1234).unwrap_err(),
                code
            );
        }
    }
    #[test]
    fn ambiguous_headers_and_large_bodies_are_rejected() {
        assert_eq!(
            parse(
                &mut raw("Content-Length: 1\r\n").as_slice(),
                "fixture-token",
                1234
            )
            .unwrap_err(),
            400
        );
        let input = String::from_utf8(raw("")).unwrap().replace(
            &format!("Content-Length: {}", BODY.len()),
            "Content-Length: 1000000",
        );
        assert_eq!(
            parse(&mut input.as_bytes(), "fixture-token", 1234).unwrap_err(),
            413
        );
        assert_eq!(
            parse(
                &mut raw("Transfer-Encoding: chunked\r\n").as_slice(),
                "fixture-token",
                1234
            )
            .unwrap_err(),
            403
        );
    }
    #[test]
    fn invalid_json_and_wrong_events_cannot_reach_ui() {
        let input = String::from_utf8(raw(""))
            .unwrap()
            .replace("PermissionRequest", "PermissionDeniedX");
        assert!(parse(&mut input.as_bytes(), "fixture-token", 1234).is_err());
    }
    #[test]
    fn live_loopback_has_no_cors_and_authenticates() {
        let service = Service::start("fixture-token".into(), |_| Ok(json!({"ok":true}))).unwrap();
        let request = String::from_utf8(raw(""))
            .unwrap()
            .replace("127.0.0.1:1234", &service.address.to_string());
        let mut stream = TcpStream::connect(service.address).unwrap();
        stream.write_all(request.as_bytes()).unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        assert!(response.starts_with("HTTP/1.1 200"));
        assert!(!response.contains("Access-Control-Allow-Origin"));
        assert!(response.contains("\"ok\":true"));
    }
    #[test]
    fn slow_headers_expire_without_reaching_handler() {
        let service = Service::start("fixture-token".into(), |_| {
            panic!("incomplete request reached UI")
        })
        .unwrap();
        let mut stream = TcpStream::connect(service.address).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        stream.write_all(b"POST /permission HTTP/1.1\r\n").unwrap();
        let start = Instant::now();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        assert!(start.elapsed() >= Duration::from_secs(1));
        assert!(start.elapsed() < Duration::from_secs(5));
        assert!(response.starts_with("HTTP/1.1 408"));
    }
    #[test]
    fn fragmented_valid_body_is_not_rejected_as_would_block() {
        let service = Service::start("fixture-token".into(), |_| Ok(json!({"ok":true}))).unwrap();
        let request = String::from_utf8(raw(""))
            .unwrap()
            .replace("127.0.0.1:1234", &service.address.to_string());
        let (header, body) = request.split_once("\r\n\r\n").unwrap();
        let mut stream = TcpStream::connect(service.address).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(4)))
            .unwrap();
        stream
            .write_all(format!("{header}\r\n\r\n").as_bytes())
            .unwrap();
        thread::sleep(Duration::from_millis(80));
        stream.write_all(body.as_bytes()).unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        assert!(response.starts_with("HTTP/1.1 200"));
    }
    #[test]
    fn overload_is_bounded_and_returns_busy() {
        use std::sync::{atomic::AtomicUsize, Condvar};
        let gate = Arc::new((Mutex::new(false), Condvar::new()));
        let count = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let (worker_gate, worker_count, worker_peak) = (gate.clone(), count.clone(), peak.clone());
        let service = Service::start("fixture-token".into(), move |_| {
            let active = worker_count.fetch_add(1, Ordering::SeqCst) + 1;
            worker_peak.fetch_max(active, Ordering::SeqCst);
            let (lock, cv) = &*worker_gate;
            let mut open = lock.lock().unwrap();
            while !*open {
                let r = cv.wait_timeout(open, Duration::from_secs(5)).unwrap();
                open = r.0;
                if r.1.timed_out() {
                    break;
                }
            }
            worker_count.fetch_sub(1, Ordering::SeqCst);
            Ok(json!({}))
        })
        .unwrap();
        let request = String::from_utf8(raw(""))
            .unwrap()
            .replace("127.0.0.1:1234", &service.address.to_string());
        let (tx, rx) = mpsc::channel();
        let mut clients = Vec::new();
        for _ in 0..32 {
            let (tx, request, address) = (tx.clone(), request.clone(), service.address);
            clients.push(thread::spawn(move || {
                let mut s = TcpStream::connect(address).unwrap();
                s.set_read_timeout(Some(Duration::from_secs(6))).unwrap();
                let _ = s.write_all(request.as_bytes());
                let mut reply = String::new();
                let _ = s.read_to_string(&mut reply);
                let _ = tx.send(reply);
            }));
        }
        let first = rx.recv_timeout(Duration::from_secs(4)).unwrap();
        *gate.0.lock().unwrap() = true;
        gate.1.notify_all();
        assert!(
            first.starts_with("HTTP/1.1 503"),
            "overloaded server did not reject excess work"
        );
        for client in clients {
            client.join().unwrap();
        }
        assert!(
            peak.load(Ordering::SeqCst) <= 8,
            "active worker count exceeded bounded capacity"
        );
    }
}
