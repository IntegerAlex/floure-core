//! Local control server: the Wayland trigger and CLI surface.
//!
//! Bound to loopback only. `floure --toggle` and compositor keybindings POST
//! here, so no second process ever starts a second audio engine.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use floure_core::Engine;
use floure_core::EngineState;

pub const ADDR: &str = "127.0.0.1:17833";

static SHOW_REQUESTED: AtomicBool = AtomicBool::new(false);
static DICTATE_REQUESTED: AtomicBool = AtomicBool::new(false);
static CONFIG_REQUESTED: AtomicBool = AtomicBool::new(false);
static WIDGET_REQUESTED: AtomicBool = AtomicBool::new(false);

pub fn request_show() {
    SHOW_REQUESTED.store(true, Ordering::SeqCst);
}

pub fn request_dictate() {
    DICTATE_REQUESTED.store(true, Ordering::SeqCst);
}

pub fn request_config() {
    CONFIG_REQUESTED.store(true, Ordering::SeqCst);
}

pub fn request_widget() {
    WIDGET_REQUESTED.store(true, Ordering::SeqCst);
}

pub fn take_show_request() -> bool {
    SHOW_REQUESTED.swap(false, Ordering::SeqCst)
}

pub fn take_dictate_request() -> bool {
    DICTATE_REQUESTED.swap(false, Ordering::SeqCst)
}

pub fn take_config_request() -> bool {
    CONFIG_REQUESTED.swap(false, Ordering::SeqCst)
}

pub fn take_widget_request() -> bool {
    WIDGET_REQUESTED.swap(false, Ordering::SeqCst)
}

/// Start the server. Returns an error if the port is already bound, which is
/// how a second instance notices the first one.
pub fn spawn(engine: Arc<Engine>) -> std::io::Result<()> {
    let listener = TcpListener::bind(ADDR)?;
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { continue };
            let engine = engine.clone();
            std::thread::spawn(move || {
                if let Err(error) = handle(stream, &engine) {
                    eprintln!("[control] {error}");
                }
            });
        }
    });
    Ok(())
}

fn handle(mut stream: TcpStream, engine: &Engine) -> std::io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let path = parts
        .next()
        .unwrap_or_default()
        .split('?')
        .next()
        .unwrap_or("");

    // Drain headers so the client sees a complete request/response cycle.
    let mut header = String::new();
    loop {
        header.clear();
        if reader.read_line(&mut header)? == 0 || header == "\r\n" || header == "\n" {
            break;
        }
    }

    let (status, body) = match (method, path) {
        ("GET", "/status") => (
            "200 OK",
            status_json(engine.state(), engine.is_warming(), engine.is_ready()),
        ),
        ("POST", "/toggle") => match engine.state() {
            EngineState::Listening | EngineState::Stopping => {
                engine.stop();
                ("200 OK", r#"{"ok":true,"action":"stop"}"#.to_string())
            }
            _ => match engine.start() {
                Ok(()) => ("200 OK", r#"{"ok":true,"action":"start"}"#.to_string()),
                Err(error) => (
                    "409 Conflict",
                    serde_json::json!({"ok": false, "error": error.to_string()}).to_string(),
                ),
            },
        },
        ("POST", "/start") => match engine.start() {
            Ok(()) => ("200 OK", r#"{"ok":true}"#.to_string()),
            Err(error) => (
                "409 Conflict",
                serde_json::json!({"ok": false, "error": error.to_string()}).to_string(),
            ),
        },
        ("POST", "/stop") => {
            engine.stop();
            ("200 OK", r#"{"ok":true}"#.to_string())
        }
        ("POST", "/show") => {
            request_show();
            ("200 OK", r#"{"ok":true}"#.to_string())
        }
        _ => (
            "404 Not Found",
            r#"{"ok":false,"error":"not found"}"#.to_string(),
        ),
    };

    write!(stream, "HTTP/1.1 {status}\r\n")?;
    write!(stream, "Content-Type: application/json\r\n")?;
    write!(stream, "Content-Length: {}\r\n", body.len())?;
    write!(stream, "Connection: close\r\n\r\n")?;
    stream.write_all(body.as_bytes())?;
    stream.flush()
}

fn status_json(state: EngineState, warming: bool, ready: bool) -> String {
    serde_json::json!({
        "ok": true,
        "state": state_name(state),
        "warming": warming,
        "ready": ready,
        "recording": matches!(state, EngineState::Listening),
    })
    .to_string()
}

fn state_name(state: EngineState) -> &'static str {
    match state {
        EngineState::Idle => "idle",
        EngineState::Listening => "listening",
        EngineState::Stopping => "stopping",
        EngineState::Transcribing => "transcribing",
        EngineState::Rewriting => "rewriting",
        EngineState::Error => "error",
        EngineState::NoModel => "no_model",
        EngineState::Warming => "warming",
        EngineState::Stopped => "stopped",
    }
}

/// Send one command to a running instance. Used by `floure --toggle` and
/// friends before GPUI is initialised.
pub fn request(method: &str, path: &str) -> std::io::Result<String> {
    request_at(ADDR, method, path)
}

fn request_at(addr: &str, method: &str, path: &str) -> std::io::Result<String> {
    let mut stream = TcpStream::connect(addr)?;
    stream.set_read_timeout(Some(Duration::from_secs(3)))?;
    stream.set_write_timeout(Some(Duration::from_secs(3)))?;
    write!(
        stream,
        "{method} {path} HTTP/1.1\r\nHost: {ADDR}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    )?;
    stream.flush()?;
    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    let body = response
        .split_once("\r\n\r\n")
        .map(|(_, body)| body.to_string())
        .unwrap_or(response);
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_payload_names_every_state() {
        for state in [
            EngineState::Idle,
            EngineState::Listening,
            EngineState::Stopping,
            EngineState::Transcribing,
            EngineState::Rewriting,
            EngineState::Error,
            EngineState::NoModel,
            EngineState::Warming,
            EngineState::Stopped,
        ] {
            let json: serde_json::Value =
                serde_json::from_str(&status_json(state, false, true)).unwrap();
            assert_eq!(json["ok"], true);
            assert_eq!(json["recording"], state == EngineState::Listening);
            assert!(json["state"].as_str().is_some_and(|s| !s.is_empty()));
        }
    }

    /// The CLI must never start a second application: it only speaks HTTP.
    #[test]
    fn cli_request_reads_the_response_body() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut request = String::new();
            reader.read_line(&mut request).unwrap();
            assert!(request.starts_with("GET /status "));
            loop {
                let mut header = String::new();
                if reader.read_line(&mut header).unwrap() == 0 || header == "\r\n" {
                    break;
                }
            }
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: 11\r\nConnection: close\r\n\r\n{{\"ok\":true}}"
            )
            .unwrap();
        });
        let body = request_at(&addr.to_string(), "GET", "/status").unwrap();
        assert_eq!(body, r#"{"ok":true}"#);
    }

    #[test]
    fn show_requests_are_one_shot() {
        request_show();
        assert!(take_show_request());
        assert!(!take_show_request());
    }
}
