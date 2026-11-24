//! RTMP / RTMPS publisher không dùng thư viện ngoài. RTMPS dùng javax.net.ssl (SSLSocket) qua JNI.
//! Video H.264 (FLV chuẩn) và H.265 (Enhanced RTMP, FourCC hvc1); âm thanh AAC.
use crate::export::{self, Jvm};
use jni::objects::{GlobalRef, JValue};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

type Res<T> = Result<T, String>;

fn es<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

enum Conn {
    Tcp(TcpStream),
    Tls { jvm: Jvm, sock: GlobalRef, out: GlobalRef, inp: GlobalRef },
}

impl Conn {
    fn tls(jvm: Jvm, host: &str, port: i32) -> Res<Conn> {
        let host = host.to_string();
        export::run(jvm, move |env, _act| {
            let plain = env.new_object("java/net/Socket", "()V", &[])?;
            let h = env.new_string(&host)?;
            let addr = env.new_object("java/net/InetSocketAddress", "(Ljava/lang/String;I)V", &[JValue::from(&h), JValue::Int(port)])?;
            env.call_method(&plain, "connect", "(Ljava/net/SocketAddress;I)V", &[JValue::from(&addr), JValue::Int(8000)])?;
            let fac = env.call_static_method("javax/net/ssl/SSLSocketFactory", "getDefault", "()Ljavax/net/SocketFactory;", &[])?.l()?;
            // bọc socket thường để có SNI + kiểm tra tên máy chủ
            let ssl = env
                .call_method(
                    &fac,
                    "createSocket",
                    "(Ljava/net/Socket;Ljava/lang/String;IZ)Ljava/net/Socket;",
                    &[JValue::from(&plain), JValue::from(&h), JValue::Int(port), JValue::Bool(1)],
                )?
                .l()?;
            let params = env.call_method(&ssl, "getSSLParameters", "()Ljavax/net/ssl/SSLParameters;", &[])?.l()?;
            let https = env.new_string("HTTPS")?;
            env.call_method(&params, "setEndpointIdentificationAlgorithm", "(Ljava/lang/String;)V", &[JValue::from(&https)])?;
            env.call_method(&ssl, "setSSLParameters", "(Ljavax/net/ssl/SSLParameters;)V", &[JValue::from(&params)])?;
            env.call_method(&ssl, "setSoTimeout", "(I)V", &[JValue::Int(12000)])?;
            env.call_method(&ssl, "startHandshake", "()V", &[])?;
            let out = env.call_method(&ssl, "getOutputStream", "()Ljava/io/OutputStream;", &[])?.l()?;
            let inp = env.call_method(&ssl, "getInputStream", "()Ljava/io/InputStream;", &[])?.l()?;
            Ok((env.new_global_ref(&ssl)?, env.new_global_ref(&out)?, env.new_global_ref(&inp)?))
        })
        .map(|(sock, out, inp)| Conn::Tls { jvm, sock, out, inp })
    }

    fn write_all(&mut self, b: &[u8]) -> Res<()> {
        match self {
            Conn::Tcp(s) => s.write_all(b).map_err(es),
            Conn::Tls { jvm, out, .. } => {
                let out = out.clone();
                export::run(*jvm, |env, _| {
                    for ch in b.chunks(16384) {
                        env.with_local_frame(8, |env| -> jni::errors::Result<()> {
                            let arr = env.byte_array_from_slice(ch)?;
                            env.call_method(&out, "write", "([BII)V", &[JValue::from(&arr), JValue::Int(0), JValue::Int(ch.len() as i32)])?;
                            Ok(())
                        })?;
                    }
                    env.call_method(&out, "flush", "()V", &[])?;
                    Ok(())
                })
            }
        }
    }

    fn read_exact(&mut self, buf: &mut [u8]) -> Res<()> {
        match self {
            Conn::Tcp(s) => s.read_exact(buf).map_err(es),
            Conn::Tls { jvm, inp, .. } => {
                let mut got = 0usize;
                while got < buf.len() {
                    let want = (buf.len() - got).min(8192);
                    let inp = inp.clone();
                    let (n, tmp) = export::run(*jvm, move |env, _| {
                        let arr = env.new_byte_array(want as i32)?;
                        let n = env.call_method(&inp, "read", "([BII)I", &[JValue::from(&arr), JValue::Int(0), JValue::Int(want as i32)])?.i()?;
                        if n > 0 {
                            let mut tmp = vec![0i8; n as usize];
                            env.get_byte_array_region(&arr, 0, &mut tmp)?;
                            Ok((n, tmp))
                        } else {
                            Ok((n, Vec::new()))
                        }
                    })?;
                    if n <= 0 {
                        return Err("Máy chủ đóng kết nối".to_string());
                    }
                    for (i, b) in tmp.iter().enumerate() {
                        buf[got + i] = *b as u8;
                    }
                    got += n as usize;
                }
                Ok(())
            }
        }
    }

    fn close(&mut self) {
        match self {
            Conn::Tcp(s) => {
                let _ = s.shutdown(std::net::Shutdown::Both);
            }
            Conn::Tls { jvm, sock, .. } => {
                let sock = sock.clone();
                let _ = export::run(*jvm, move |env, _| {
                    env.call_method(&sock, "close", "()V", &[])?;
                    Ok(())
                });
            }
        }
    }
}

// ---------- AMF0 ----------
fn amf_num(o: &mut Vec<u8>, v: f64) {
    o.push(0x00);
    o.extend_from_slice(&v.to_be_bytes());
}
fn amf_str(o: &mut Vec<u8>, s: &str) {
    o.push(0x02);
    o.extend_from_slice(&(s.len() as u16).to_be_bytes());
    o.extend_from_slice(s.as_bytes());
}
fn amf_null(o: &mut Vec<u8>) {
    o.push(0x05);
}
fn amf_key(o: &mut Vec<u8>, k: &str) {
    o.extend_from_slice(&(k.len() as u16).to_be_bytes());
    o.extend_from_slice(k.as_bytes());
}
fn amf_obj_end(o: &mut Vec<u8>) {
    o.extend_from_slice(&[0, 0, 9]);
}

#[derive(Default)]
struct St {
    len: usize,
    mtype: u8,
    buf: Vec<u8>,
}

pub struct Rtmp {
    conn: Conn,
    out_chunk: usize,
    in_chunk: usize,
    stream_id: u32,
    parts: HashMap<u32, St>,
    pub sent: u64,
}

pub struct Meta {
    pub w: i32,
    pub h: i32,
    pub fps: i32,
    pub vkbps: i32,
    pub akbps: i32,
    pub audio: bool,
    pub hevc: bool,
}

/// rtmp://host[:port]/app[/...]  hoặc rtmps://...; trả (tls, host, port, app, tcUrl, key trong URL nếu có)
fn parse_url(url: &str, key: &str) -> Res<(bool, String, i32, String, String, String)> {
    let u = url.trim();
    let (tls, rest) = if let Some(r) = u.strip_prefix("rtmps://") {
        (true, r)
    } else if let Some(r) = u.strip_prefix("rtmp://") {
        (false, r)
    } else {
        return Err("URL phải bắt đầu bằng rtmp:// hoặc rtmps://".to_string());
    };
    let (hostport, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i + 1..]),
        None => return Err("URL thiếu tên ứng dụng (ví dụ rtmp://host/live)".to_string()),
    };
    let (host, port) = match hostport.rsplit_once(':') {
        Some((h, p)) => (h.to_string(), p.parse::<i32>().map_err(|_| "Port không hợp lệ".to_string())?),
        None => (hostport.to_string(), if tls { 443 } else { 1935 }),
    };
    let path = path.trim_end_matches('/');
    let (app, key_in_url) = match path.split_once('/') {
        Some((a, k)) => (a.to_string(), k.to_string()),
        None => (path.to_string(), String::new()),
    };
    let key = if key.trim().is_empty() { key_in_url } else { key.trim().to_string() };
    if key.is_empty() {
        return Err("Chưa nhập khoá luồng (stream key)".to_string());
    }
    let scheme = if tls { "rtmps" } else { "rtmp" };
    let tc = format!("{}://{}:{}/{}", scheme, host, port, app);
    Ok((tls, host, port, app, tc, key))
}

impl Rtmp {
    pub fn connect(url: &str, key: &str, jvm: Jvm, m: &Meta) -> Res<Rtmp> {
        let (tls, host, port, app, tc, key) = parse_url(url, key)?;
        let conn = if tls {
            Conn::tls(jvm, &host, port)?
        } else {
            use std::net::ToSocketAddrs;
            let addr = (host.as_str(), port as u16).to_socket_addrs().map_err(es)?.next().ok_or_else(|| "Không phân giải được địa chỉ".to_string())?;
            let s = TcpStream::connect_timeout(&addr, Duration::from_secs(8)).map_err(es)?;
            s.set_read_timeout(Some(Duration::from_secs(12))).map_err(es)?;
            s.set_write_timeout(Some(Duration::from_secs(12))).map_err(es)?;
            let _ = s.set_nodelay(true);
            Conn::Tcp(s)
        };
        let mut r = Rtmp { conn, out_chunk: 128, in_chunk: 128, stream_id: 0, parts: HashMap::new(), sent: 0 };
        r.handshake()?;
        // chunk size lớn hơn
        r.send_msg(2, 1, 0, 0, &4096u32.to_be_bytes())?;
        r.out_chunk = 4096;
        // connect
        let mut c = Vec::new();
        amf_str(&mut c, "connect");
        amf_num(&mut c, 1.0);
        c.push(0x03);
        amf_key(&mut c, "app");
        amf_str(&mut c, &app);
        amf_key(&mut c, "type");
        amf_str(&mut c, "nonprivate");
        amf_key(&mut c, "flashVer");
        amf_str(&mut c, "FMLE/3.0 (compatible; CSar)");
        amf_key(&mut c, "tcUrl");
        amf_str(&mut c, &tc);
        if m.hevc {
            amf_key(&mut c, "fourCcList");
            c.push(0x0A);
            c.extend_from_slice(&1u32.to_be_bytes());
            amf_str(&mut c, "hvc1");
        }
        amf_obj_end(&mut c);
        r.send_msg(3, 20, 0, 0, &c)?;
        r.wait_cmd("connect")?;
        let mut a = Vec::new();
        amf_str(&mut a, "releaseStream");
        amf_num(&mut a, 2.0);
        amf_null(&mut a);
        amf_str(&mut a, &key);
        r.send_msg(3, 20, 0, 0, &a)?;
        let mut a = Vec::new();
        amf_str(&mut a, "FCPublish");
        amf_num(&mut a, 3.0);
        amf_null(&mut a);
        amf_str(&mut a, &key);
        r.send_msg(3, 20, 0, 0, &a)?;
        let mut a = Vec::new();
        amf_str(&mut a, "createStream");
        amf_num(&mut a, 4.0);
        amf_null(&mut a);
        r.send_msg(3, 20, 0, 0, &a)?;
        let res = r.wait_cmd("createStream")?;
        r.stream_id = parse_stream_id(&res).unwrap_or(1);
        let mut a = Vec::new();
        amf_str(&mut a, "publish");
        amf_num(&mut a, 5.0);
        amf_null(&mut a);
        amf_str(&mut a, &key);
        amf_str(&mut a, "live");
        let sid = r.stream_id;
        r.send_msg(4, 20, sid, 0, &a)?;
        r.wait_publish()?;
        // metadata
        let mut d = Vec::new();
        amf_str(&mut d, "@setDataFrame");
        amf_str(&mut d, "onMetaData");
        d.push(0x03);
        amf_key(&mut d, "width");
        amf_num(&mut d, m.w as f64);
        amf_key(&mut d, "height");
        amf_num(&mut d, m.h as f64);
        amf_key(&mut d, "framerate");
        amf_num(&mut d, m.fps as f64);
        amf_key(&mut d, "videodatarate");
        amf_num(&mut d, m.vkbps as f64);
        if !m.hevc {
            amf_key(&mut d, "videocodecid");
            amf_num(&mut d, 7.0);
        }
        if m.audio {
            amf_key(&mut d, "audiocodecid");
            amf_num(&mut d, 10.0);
            amf_key(&mut d, "audiodatarate");
            amf_num(&mut d, m.akbps as f64);
            amf_key(&mut d, "audiosamplerate");
            amf_num(&mut d, 44100.0);
            amf_key(&mut d, "audiosamplesize");
            amf_num(&mut d, 16.0);
            amf_key(&mut d, "stereo");
            d.extend_from_slice(&[0x01, 0x00]);
        }
        amf_key(&mut d, "encoder");
        amf_str(&mut d, "CSar");
        amf_obj_end(&mut d);
        r.send_msg(4, 18, sid, 0, &d)?;
        Ok(r)
    }

    fn handshake(&mut self) -> Res<()> {
        let mut c1 = vec![0u8; 1537];
        c1[0] = 3;
        // c1[1..5] = thời gian 0, c1[5..9] = 0, phần còn lại ngẫu nhiên (đủ dùng: lấy từ đồng hồ)
        let mut seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(1) | 1;
        for b in c1[9..].iter_mut() {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            *b = (seed & 0xff) as u8;
        }
        self.conn.write_all(&c1)?;
        let mut s0s1s2 = vec![0u8; 1 + 1536 + 1536];
        self.conn.read_exact(&mut s0s1s2)?;
        if s0s1s2[0] != 3 {
            return Err("Máy chủ RTMP trả phiên bản lạ".to_string());
        }
        let s1 = s0s1s2[1..1537].to_vec();
        self.conn.write_all(&s1)?; // C2 = S1
        Ok(())
    }

    fn send_msg(&mut self, csid: u8, mtype: u8, msid: u32, ts: u32, payload: &[u8]) -> Res<()> {
        let ext = ts >= 0xFF_FFFF;
        let ts24 = if ext { 0xFF_FFFFu32 } else { ts };
        let mut out: Vec<u8> = Vec::with_capacity(payload.len() + payload.len() / self.out_chunk + 20);
        out.push(csid & 0x3f);
        out.extend_from_slice(&ts24.to_be_bytes()[1..]);
        out.extend_from_slice(&(payload.len() as u32).to_be_bytes()[1..]);
        out.push(mtype);
        out.extend_from_slice(&msid.to_le_bytes());
        if ext {
            out.extend_from_slice(&ts.to_be_bytes());
        }
        let mut off = 0usize;
        let mut first = true;
        while off < payload.len() || first {
            let n = (payload.len() - off).min(self.out_chunk);
            if !first {
                out.push(0xC0 | (csid & 0x3f));
                if ext {
                    out.extend_from_slice(&ts.to_be_bytes());
                }
            }
            out.extend_from_slice(&payload[off..off + n]);
            off += n;
            first = false;
            if n == 0 {
                break;
            }
        }
        self.sent += out.len() as u64;
        self.conn.write_all(&out)
    }

    fn read_msg(&mut self) -> Res<(u8, Vec<u8>)> {
        loop {
            let mut b = [0u8; 1];
            self.conn.read_exact(&mut b)?;
            let fmt = b[0] >> 6;
            let mut csid = (b[0] & 0x3f) as u32;
            if csid == 0 {
                let mut x = [0u8; 1];
                self.conn.read_exact(&mut x)?;
                csid = x[0] as u32 + 64;
            } else if csid == 1 {
                let mut x = [0u8; 2];
                self.conn.read_exact(&mut x)?;
                csid = x[0] as u32 + x[1] as u32 * 256 + 64;
            }
            let mut st = self.parts.remove(&csid).unwrap_or_default();
            let hdr_len = match fmt {
                0 => 11,
                1 => 7,
                2 => 3,
                _ => 0,
            };
            let mut h = [0u8; 11];
            if hdr_len > 0 {
                self.conn.read_exact(&mut h[..hdr_len])?;
            }
            let tsf = if hdr_len >= 3 { ((h[0] as u32) << 16) | ((h[1] as u32) << 8) | h[2] as u32 } else { 0 };
            if fmt == 0 || fmt == 1 {
                st.len = ((h[3] as usize) << 16) | ((h[4] as usize) << 8) | h[5] as usize;
                st.mtype = h[6];
                st.buf.clear();
            }
            if tsf == 0xFF_FFFF {
                let mut e = [0u8; 4];
                self.conn.read_exact(&mut e)?;
            }
            let remaining = st.len.saturating_sub(st.buf.len());
            let n = remaining.min(self.in_chunk);
            let mut chunk = vec![0u8; n];
            if n > 0 {
                self.conn.read_exact(&mut chunk)?;
            }
            st.buf.extend_from_slice(&chunk);
            if st.buf.len() >= st.len {
                let payload = std::mem::take(&mut st.buf);
                let t = st.mtype;
                if t == 1 && payload.len() >= 4 {
                    self.in_chunk = (u32::from_be_bytes([payload[0], payload[1], payload[2], payload[3]]) as usize).clamp(1, 1 << 20);
                }
                self.parts.insert(csid, st);
                return Ok((t, payload));
            }
            self.parts.insert(csid, st);
        }
    }

    fn cmd_name(p: &[u8]) -> Option<String> {
        if p.len() >= 3 && p[0] == 0x02 {
            let l = ((p[1] as usize) << 8) | p[2] as usize;
            if p.len() >= 3 + l {
                return Some(String::from_utf8_lossy(&p[3..3 + l]).to_string());
            }
        }
        None
    }

    fn wait_cmd(&mut self, what: &str) -> Res<Vec<u8>> {
        for _ in 0..64 {
            let (t, p) = self.read_msg()?;
            if t == 20 {
                match Rtmp::cmd_name(&p).as_deref() {
                    Some("_result") => return Ok(p),
                    Some("_error") => return Err(format!("Máy chủ từ chối lệnh {}", what)),
                    _ => {}
                }
            }
        }
        Err(format!("Không nhận được phản hồi cho {}", what))
    }

    fn wait_publish(&mut self) -> Res<()> {
        for _ in 0..64 {
            let (t, p) = self.read_msg()?;
            if t == 20 && Rtmp::cmd_name(&p).as_deref() == Some("onStatus") {
                let s = String::from_utf8_lossy(&p).to_string();
                if s.contains("Publish.Start") {
                    return Ok(());
                }
                if let Some(i) = s.find("NetStream.") {
                    let code: String = s[i..].chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '.').collect();
                    return Err(format!("Máy chủ báo: {}", code));
                }
            }
        }
        Err("Không nhận được xác nhận publish".to_string())
    }

    pub fn video(&mut self, ts: u32, payload: &[u8]) -> Res<()> {
        let sid = self.stream_id;
        self.send_msg(6, 9, sid, ts, payload)
    }

    pub fn audio(&mut self, ts: u32, payload: &[u8]) -> Res<()> {
        let sid = self.stream_id;
        self.send_msg(7, 8, sid, ts, payload)
    }

    pub fn close(&mut self) {
        let sid = self.stream_id;
        let mut a = Vec::new();
        amf_str(&mut a, "deleteStream");
        amf_num(&mut a, 6.0);
        amf_null(&mut a);
        amf_num(&mut a, sid as f64);
        let _ = self.send_msg(3, 20, 0, 0, &a);
        self.conn.close();
    }
}

fn parse_stream_id(p: &[u8]) -> Option<u32> {
    // "_result", txn(number), null/object, id(number)
    let mut i = 0usize;
    if p.get(i)? != &0x02 {
        return None;
    }
    i += 3 + (((p[1] as usize) << 8) | p[2] as usize);
    if p.get(i)? != &0x00 {
        return None;
    }
    i += 9;
    match p.get(i)? {
        0x05 => i += 1,
        _ => return None,
    }
    if p.get(i)? != &0x00 || i + 9 > p.len() {
        return None;
    }
    let mut b = [0u8; 8];
    b.copy_from_slice(&p[i + 1..i + 9]);
    Some(f64::from_be_bytes(b) as u32)
}
