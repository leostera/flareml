//! Minimal, read-only HTTP/1.1 transport: one bounded request per connection.
use flareml::explorer::Session;
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
const MAX_RESPONSE: usize = 8_000_000;
const CSP: &str = "default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; connect-src 'self'; img-src 'self' data:; font-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'none'";

pub fn serve(session: Session, no_open: bool) -> Result<(), String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let host = listener
        .local_addr()
        .map_err(|e| e.to_string())?
        .to_string();
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|e| e.to_string())?;
    let token: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    let url = format!("http://{host}/#{token}");
    let running = Arc::new(AtomicBool::new(true));
    let flag = running.clone();
    ctrlc::set_handler(move || flag.store(false, Ordering::Relaxed)).map_err(|e| e.to_string())?;
    eprintln!(
        "Explorer: {url}\nValidated evidence only. Ctrl-C to stop; keep this session URL private."
    );
    if !no_open {
        open_browser(url);
    }
    while running.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((mut stream, _)) => {
                let _ = connection(&mut stream, &host, &token, &session);
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(20))
            }
            Err(e) => return Err(e.to_string()),
        }
    }
    Ok(())
}
fn open_browser(url: String) {
    std::thread::spawn(move || {
        #[cfg(target_os = "macos")]
        let result = std::process::Command::new("open").arg(&url).status();
        #[cfg(target_os = "windows")]
        let result = std::process::Command::new("rundll32")
            .args(["url.dll,FileProtocolHandler", &url])
            .status();
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        let result = std::process::Command::new("xdg-open").arg(&url).status();
        if !result.is_ok_and(|s| s.success()) {
            eprintln!("Could not open a browser; open the printed Explorer URL manually.");
        }
    });
}
fn connection(
    stream: &mut TcpStream,
    host: &str,
    token: &str,
    session: &Session,
) -> std::io::Result<()> {
    // Accepted sockets can inherit nonblocking mode on some platforms.
    // Header reads use a deadline, not speculative WouldBlock failures.
    stream.set_nonblocking(false)?;
    let start = Instant::now();
    let mut data = Vec::new();
    while data.len() < 8192 && !data.ends_with(b"\r\n\r\n") {
        let remaining = Duration::from_secs(2)
            .checked_sub(start.elapsed())
            .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::TimedOut))?;
        stream.set_read_timeout(Some(remaining.max(Duration::from_millis(1))))?;
        let mut byte = [0];
        if stream.read(&mut byte)? == 0 {
            return Ok(());
        }
        data.push(byte[0]);
    }
    let (status, mime, mut body) = if data.ends_with(b"\r\n\r\n") {
        response(
            std::str::from_utf8(&data).unwrap_or_default(),
            host,
            token,
            session,
        )
    } else {
        (431, "text/plain", b"Request headers too large".to_vec())
    };
    let status = if body.len() > MAX_RESPONSE {
        body = b"Explorer rendering limit: response exceeds 8 MB; use text replay".to_vec();
        413
    } else {
        status
    };
    let header = format!(
        "HTTP/1.1 {status} {}\r\nContent-Type: {mime}; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nReferrer-Policy: no-referrer\r\nContent-Security-Policy: {CSP}\r\n\r\n",
        if status == 200 { "OK" } else { "Error" },
        body.len()
    );
    // One overall bounded write, even if a peer reads very slowly.
    stream.set_nonblocking(true)?;
    let deadline = Instant::now() + Duration::from_secs(2);
    let bytes = [header.as_bytes(), &body].concat();
    let mut offset = 0;
    while offset < bytes.len() {
        if Instant::now() >= deadline {
            return Err(std::io::ErrorKind::TimedOut.into());
        }
        match stream.write(&bytes[offset..]) {
            Ok(0) => return Ok(()),
            Ok(n) => offset += n,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(2))
            }
            Err(e) => return Err(e),
        }
    }
    Ok(())
}
fn response(
    request: &str,
    host: &str,
    token: &str,
    session: &Session,
) -> (u16, &'static str, Vec<u8>) {
    let fail = |code, msg: &str| (code, "text/plain", msg.as_bytes().to_vec());
    let mut lines = request.split("\r\n");
    let parts: Vec<_> = lines.next().unwrap_or_default().split(' ').collect();
    if parts.len() != 3 || parts[2] != "HTTP/1.1" {
        return fail(400, "Invalid request");
    }
    if parts[0] != "GET" {
        return fail(405, "Read-only session");
    }
    let mut headers = BTreeMap::new();
    for line in lines.take_while(|s| !s.is_empty()) {
        let Some((name, value)) = line.split_once(':') else {
            return fail(400, "Invalid header");
        };
        if headers
            .insert(name.to_ascii_lowercase(), value.trim())
            .is_some()
        {
            return fail(400, "Duplicate header");
        }
    }
    if headers.get("host") != Some(&host)
        || headers
            .get("origin")
            .is_some_and(|s| *s != format!("http://{host}"))
        || headers
            .get("sec-fetch-site")
            .is_some_and(|s| *s == "cross-site")
    {
        return fail(403, "Origin rejected");
    }
    if headers.contains_key("transfer-encoding")
        || headers.get("content-length").is_some_and(|n| *n != "0")
    {
        return fail(400, "Request body not allowed");
    }
    let path = parts[1];
    let asset: Option<(&str, &[u8])> = match path {
        "/" => Some(("text/html", include_bytes!("../explorer/dist/index.html"))),
        "/assets/app.js" => Some((
            "text/javascript",
            include_bytes!("../explorer/dist/assets/app.js"),
        )),
        "/assets/app.css" => Some((
            "text/css",
            include_bytes!("../explorer/dist/assets/app.css"),
        )),
        "/THIRD_PARTY_NOTICES.txt" => Some((
            "text/plain",
            include_bytes!("../explorer/dist/THIRD_PARTY_NOTICES.txt"),
        )),
        _ => None,
    };
    if let Some((mime, data)) = asset {
        return (200, mime, data.to_vec());
    }
    if headers.get("authorization") != Some(&format!("Bearer {token}").as_str()) {
        return fail(403, "Session authorization required");
    }
    let value = if path == "/api/tree" {
        Some(session.tree())
    } else if let Some(rest) = path.strip_prefix("/api/traces/") {
        let Some((id, resource)) = rest.split_once('/') else {
            return fail(404, "Unknown execution");
        };
        let Some(execution) = id.parse().ok().and_then(|id| session.execution(id)) else {
            return fail(404, "Unknown execution");
        };
        if resource == "session" {
            Some(execution.metadata())
        } else {
            resource
                .strip_prefix("snapshot/")
                .and_then(|n| n.parse().ok())
                .and_then(|n| execution.snapshot(n))
        }
    } else if path == "/api/session" {
        Some(session.metadata())
    } else if let Some(index) = path
        .strip_prefix("/api/snapshot/")
        .and_then(|s| s.parse::<usize>().ok())
    {
        session.snapshot(index)
    } else if let Some(query) = path.strip_prefix("/api/steps?") {
        let mut offset = 0;
        let mut filter = String::new();
        for part in query.split('&') {
            let Some((key, value)) = part.split_once('=') else {
                return fail(400, "Invalid query");
            };
            match key {
                "offset" => {
                    let Ok(n) = value.parse() else {
                        return fail(400, "Invalid offset");
                    };
                    offset = n;
                }
                "filter" => {
                    let Some(decoded) = decode(value) else {
                        return fail(400, "Invalid filter");
                    };
                    filter = decoded;
                }
                _ => return fail(400, "Unknown query"),
            }
        }
        if filter.len() > 128 {
            return fail(400, "Filter too long");
        }
        Some(session.summaries(offset, &filter))
    } else {
        None
    };
    match value {
        Some(value) => (
            200,
            "application/json",
            serde_json::to_vec(&value).expect("JSON DTO"),
        ),
        None => fail(404, "Not found"),
    }
}
fn decode(value: &str) -> Option<String> {
    let mut bytes = value.bytes();
    let mut out = Vec::new();
    while let Some(byte) = bytes.next() {
        out.push(if byte == b'%' {
            let a = bytes.next()?;
            let b = bytes.next()?;
            u8::from_str_radix(std::str::from_utf8(&[a, b]).ok()?, 16).ok()?
        } else if byte == b'+' {
            b' '
        } else {
            byte
        });
    }
    String::from_utf8(out).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn session() -> Session {
        let source = include_str!("../examples/explicit-startup.fml");
        let p = flareml::compile(source, None).unwrap();
        let r = flareml::checker::check(source, &p, &Default::default()).unwrap();
        Session::validated(source.into(), r.witness().unwrap().clone(), &p).unwrap()
    }
    #[test]
    fn readonly_routes_and_session_isolation() {
        let s = session();
        let get = |path: &str, headers: &str| {
            response(
                &format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1:1234\r\n{headers}\r\n"),
                "127.0.0.1:1234",
                "secret",
                &s,
            )
            .0
        };
        assert_eq!(get("/", ""), 200);
        assert_eq!(get("/api/session", ""), 403);
        assert_eq!(get("/api/session", "Authorization: Bearer wrong\r\n"), 403);
        assert_eq!(get("/api/session", "Authorization: Bearer secret\r\n"), 200);
        assert_eq!(
            get(
                "/api/session",
                "Authorization: Bearer secret\r\nOrigin: https://evil.test\r\n"
            ),
            403
        );
        assert_eq!(
            get(
                "/api/session",
                "Authorization: Bearer secret\r\nHost: evil.test\r\n"
            ),
            400
        );
        assert_eq!(
            get("/../../Cargo.toml", "Authorization: Bearer secret\r\n"),
            404
        );
        assert_eq!(
            get("/api/snapshot/999", "Authorization: Bearer secret\r\n"),
            404
        );
        assert_eq!(
            get(
                "/api/steps?offset=0&filter=process",
                "Authorization: Bearer secret\r\n"
            ),
            200
        );
        assert_eq!(
            response("POST / HTTP/1.1\r\n\r\n", "127.0.0.1:1234", "secret", &s).0,
            405
        );
    }
}
