//! ServerApi / PhotoClient / StabClient: gửi tệp -> chờ xử lý -> tải kết quả (HTTP thuần).
use crate::cfg::Cfg;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub type Res<T> = Result<T, String>;

fn es<E: std::fmt::Display>(x: E) -> String {
    x.to_string()
}
fn bad(m: &str) -> String {
    m.to_string()
}

fn jfind<'a>(j: &'a str, key: &str) -> Option<&'a str> {
    let pat = format!("\"{}\"", key);
    let i = j.find(&pat)?;
    let rest = j[i + pat.len()..].trim_start();
    let rest = rest.strip_prefix(':')?;
    Some(rest.trim_start())
}

fn jstr(j: &str, key: &str) -> Option<String> {
    let r = jfind(j, key)?;
    let mut it = r.chars();
    if it.next()? != '"' {
        return None;
    }
    let mut out = String::new();
    while let Some(c) = it.next() {
        match c {
            '"' => return Some(out),
            '\\' => match it.next()? {
                'n' => out.push('\n'),
                't' => out.push('\t'),
                'r' => out.push('\r'),
                'u' => {
                    let h: String = it.by_ref().take(4).collect();
                    if let Some(ch) = u32::from_str_radix(&h, 16).ok().and_then(char::from_u32) {
                        out.push(ch);
                    }
                }
                o => out.push(o),
            },
            _ => out.push(c),
        }
    }
    None
}

fn jnum(j: &str, key: &str) -> Option<f64> {
    let r = jfind(j, key)?;
    let end = r
        .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-' || c == 'e' || c == 'E' || c == '+'))
        .unwrap_or(r.len());
    r[..end].parse().ok()
}

struct Base {
    host: String,
    port: u16,
    prefix: String,
}

fn parse_base(s: &str) -> Res<Base> {
    let s = s.trim();
    if s.is_empty() {
        return Err(bad("Chưa nhập địa chỉ server"));
    }
    if s.starts_with("https://") {
        return Err(bad("Chỉ hỗ trợ http:// (giống app gốc)"));
    }
    let rest = s.strip_prefix("http://").unwrap_or(s).trim_end_matches('/');
    let (hostport, prefix) = match rest.find('/') {
        Some(i) => (&rest[..i], rest[i..].to_string()),
        None => (rest, String::new()),
    };
    let (host, port) = match hostport.rsplit_once(':') {
        Some((h, p)) => (h.to_string(), p.parse::<u16>().map_err(|_| bad("Port không hợp lệ"))?),
        None => (hostport.to_string(), 80u16),
    };
    Ok(Base { host, port, prefix })
}

struct Resp {
    code: u16,
    body: Vec<u8>,
}

impl Resp {
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).to_string()
    }
    fn err_text(&self) -> String {
        let t = self.text();
        jstr(&t, "error").unwrap_or(t)
    }
}

fn hdr(headers: &[(String, String)], k: &str) -> Option<String> {
    headers.iter().find(|(a, _)| a == k).map(|(_, v)| v.clone())
}

fn read_resp(s: TcpStream, down: &mut dyn FnMut(u64, u64)) -> Res<Resp> {
    let mut r = BufReader::new(s);
    let mut line = String::new();
    r.read_line(&mut line).map_err(es)?;
    let code: u16 = line
        .split_whitespace()
        .nth(1)
        .and_then(|c| c.parse().ok())
        .ok_or_else(|| bad("Phản hồi HTTP không hợp lệ"))?;
    let mut headers: Vec<(String, String)> = Vec::new();
    loop {
        line.clear();
        r.read_line(&mut line).map_err(es)?;
        let t = line.trim_end();
        if t.is_empty() {
            break;
        }
        if let Some((k, v)) = t.split_once(':') {
            headers.push((k.trim().to_lowercase(), v.trim().to_string()));
        }
    }
    let mut body: Vec<u8> = Vec::new();
    let chunked = hdr(&headers, "transfer-encoding").map(|v| v.to_lowercase().contains("chunked")).unwrap_or(false);
    if chunked {
        loop {
            line.clear();
            r.read_line(&mut line).map_err(es)?;
            let sz_txt = line.trim().split(';').next().unwrap_or("0").to_string();
            let sz = usize::from_str_radix(&sz_txt, 16).map_err(|_| bad("Chunk không hợp lệ"))?;
            if sz == 0 {
                break;
            }
            let mut c = vec![0u8; sz];
            r.read_exact(&mut c).map_err(es)?;
            body.extend_from_slice(&c);
            let mut crlf = [0u8; 2];
            r.read_exact(&mut crlf).map_err(es)?;
            down(body.len() as u64, 0);
        }
    } else if let Some(cl) = hdr(&headers, "content-length").and_then(|v| v.parse::<u64>().ok()) {
        let mut buf = vec![0u8; 65536];
        while (body.len() as u64) < cl {
            let n = r.read(&mut buf).map_err(es)?;
            if n == 0 {
                break;
            }
            body.extend_from_slice(&buf[..n]);
            down(body.len() as u64, cl);
        }
    } else {
        r.read_to_end(&mut body).map_err(es)?;
    }
    Ok(Resp { code, body })
}

#[allow(clippy::too_many_arguments)]
fn send(
    b: &Base,
    method: &str,
    path: &str,
    tok: &str,
    ctype: Option<&str>,
    parts: &[&[u8]],
    read_to: Duration,
    up: &mut dyn FnMut(u64, u64),
    down: &mut dyn FnMut(u64, u64),
) -> Res<Resp> {
    let addr = (b.host.as_str(), b.port)
        .to_socket_addrs()
        .map_err(es)?
        .next()
        .ok_or_else(|| bad("Không phân giải được địa chỉ server"))?;
    let mut s = TcpStream::connect_timeout(&addr, Duration::from_secs(6)).map_err(es)?;
    s.set_read_timeout(Some(read_to)).map_err(es)?;
    s.set_write_timeout(Some(Duration::from_secs(60))).map_err(es)?;
    let total: u64 = parts.iter().map(|p| p.len() as u64).sum();
    let mut head = format!(
        "{} {}{} HTTP/1.1\r\nHost: {}:{}\r\nConnection: close\r\n",
        method, b.prefix, path, b.host, b.port
    );
    if !tok.is_empty() {
        head.push_str(&format!("X-Token: {}\r\n", tok));
    }
    if let Some(c) = ctype {
        head.push_str(&format!("Content-Type: {}\r\n", c));
    }
    if method == "POST" || total > 0 {
        head.push_str(&format!("Content-Length: {}\r\n", total));
    }
    head.push_str("\r\n");
    s.write_all(head.as_bytes()).map_err(es)?;
    let mut sent = 0u64;
    for p in parts {
        for ch in p.chunks(32768) {
            s.write_all(ch).map_err(es)?;
            sent += ch.len() as u64;
            up(sent, total);
        }
    }
    s.flush().map_err(es)?;
    read_resp(s, down)
}

fn pct(s: u64, t: u64) -> i32 {
    if t == 0 {
        -1
    } else {
        (s * 100 / t) as i32
    }
}

fn nanos() -> u128 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0)
}

#[allow(clippy::too_many_arguments)]
fn job_flow(
    b: &Base,
    tok: &str,
    kind: &str,
    query: &str,
    file: &[u8],
    mime: &str,
    title: &str,
    jid: &mut Option<String>,
    prog: &mut dyn FnMut(&str, i32),
) -> Res<Vec<u8>> {
    let mut noop = |_a: u64, _b: u64| {};
    let mut noop2 = |_a: u64, _b: u64| {};

    let bd = format!("----csan{}", nanos());
    let ext = if mime.starts_with("video") { "mp4" } else { "jpg" };
    let head = format!(
        "--{}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"f0.{}\"\r\nContent-Type: {}\r\n\r\n",
        bd, ext, mime
    );
    let tail = format!("\r\n--{}--\r\n", bd);
    let ctype = format!("multipart/form-data; boundary={}", bd);
    let path = if query.is_empty() {
        format!("/api/jobs?kind={}", kind)
    } else {
        format!("/api/jobs?kind={}&{}", kind, query)
    };
    let label = format!("{}: Đang gửi", title);
    let r = send(
        b,
        "POST",
        &path,
        tok,
        Some(&ctype),
        &[head.as_bytes(), file, tail.as_bytes()],
        Duration::from_secs(60),
        &mut |s: u64, t: u64| prog(&label, pct(s, t)),
        &mut noop,
    )?;
    if r.code != 200 && r.code != 202 {
        return Err(format!("Server trả lỗi {} {}", r.code, r.err_text()));
    }
    let id = jstr(&r.text(), "id").ok_or_else(|| bad("Server không trả id job"))?;
    *jid = Some(id.clone());

    let tp = Instant::now();
    loop {
        if tp.elapsed() > Duration::from_secs(15 * 60) {
            return Err(bad("Server xử lý quá lâu"));
        }
        let s = send(b, "GET", &format!("/api/jobs/{}", id), tok, None, &[], Duration::from_secs(15), &mut noop, &mut noop2)?;
        if s.code != 200 {
            return Err(format!("Không đọc được tiến độ: {} {}", s.code, s.err_text()));
        }
        let j = s.text();
        let st = jstr(&j, "state").unwrap_or_default();
        if st == "error" {
            return Err(format!("Server lỗi: {}", jstr(&j, "error").unwrap_or_default()));
        }
        if st == "done" {
            break;
        }
        if st == "queued" {
            let pos = jnum(&j, "position").unwrap_or(0.0) as i64;
            let m = if pos > 1 {
                format!("{}: Đang chờ server (hàng đợi {})", title, pos)
            } else {
                format!("{}: Đang chờ server", title)
            };
            prog(&m, -1);
        } else {
            let stage = jstr(&j, "stage").unwrap_or_default();
            let m = if stage.is_empty() {
                format!("{}: Đang xử lý", title)
            } else {
                format!("{}: Đang xử lý - {}", title, stage)
            };
            prog(&m, jnum(&j, "progress").unwrap_or(0.0) as i32);
        }
        std::thread::sleep(Duration::from_millis(400));
    }

    let label = format!("{}: Đang tải về", title);
    let d = send(
        b,
        "GET",
        &format!("/api/jobs/{}/result", id),
        tok,
        None,
        &[],
        Duration::from_secs(120),
        &mut noop,
        &mut |g: u64, t: u64| prog(&label, pct(g, t)),
    )?;
    if d.code != 200 {
        return Err(format!("Không tải được kết quả: {} {}", d.code, d.err_text()));
    }
    if d.body.len() < 512 {
        return Err(bad("Kết quả rỗng"));
    }
    Ok(d.body)
}

pub fn run_job(
    cfg: &Cfg,
    kind: &str,
    query: &str,
    file: &[u8],
    mime: &str,
    title: &str,
    prog: &mut dyn FnMut(&str, i32),
) -> Res<Vec<u8>> {
    let b = parse_base(&cfg.str("stabsrv_url", ""))?;
    let tok = cfg.str("stabsrv_tok", "").trim().to_string();
    let mut jid: Option<String> = None;
    let res = job_flow(&b, &tok, kind, query, file, mime, title, &mut jid, prog);
    if let Some(id) = &jid {
        let mut n1 = |_a: u64, _b: u64| {};
        let mut n2 = |_a: u64, _b: u64| {};
        let _ = send(&b, "DELETE", &format!("/api/jobs/{}", id), &tok, None, &[], Duration::from_secs(5), &mut n1, &mut n2);
    }
    res
}

/// PhotoClient: khử nhiễu + làm nét ảnh JPEG qua server.
pub fn photo_process(cfg: &Cfg, jpg: &[u8], prog: &mut dyn FnMut(&str, i32)) -> Res<Vec<u8>> {
    let dn = cfg.int("pden", 40).clamp(0, 100);
    let sh = cfg.int("psharp", 25).clamp(0, 100);
    run_job(cfg, "photo", &format!("denoise={}&sharpen={}", dn, sh), jpg, "image/jpeg", "Khử nhiễu ảnh", prog)
}

/// StabClient: ổn định video qua server (chưa nối vào UI vì chưa có quay video).
#[allow(dead_code)]
pub fn stab_process(cfg: &Cfg, video: &[u8], fps: f32, prog: &mut dyn FnMut(&str, i32)) -> Res<Vec<u8>> {
    let scale = cfg.int("stabscale", 95).clamp(70, 100) as f32 / 100.0;
    let q = format!(
        "zmin=1.1&zmax=1.3&fps={:.3}&res=1080&extend={}&scale={:.3}",
        fps,
        if cfg.boolean("stabext", true) { 1 } else { 0 },
        scale
    );
    run_job(cfg, "video", &q, video, "video/mp4", "Ổn định video Live", prog)
}
