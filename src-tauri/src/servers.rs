//! Live server browser. Reads each instance's `servers.dat` (the in-game
//! multiplayer list, stored as *uncompressed* NBT) and pings servers with the
//! Minecraft Server List Ping protocol so the UI can show live MOTD, player
//! counts and latency — all without launching the game.

use crate::error::{Error, Result};
use crate::state::AppState;
use serde::Serialize;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;

// ---------------------------------------------------------------------------
// servers.dat — saved multiplayer entries
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedServer {
    pub name: String,
    pub ip: String,
    /// Base64 PNG (no data-URI prefix) of the icon the game last cached, if any.
    pub icon: Option<String>,
}

pub fn list_servers(state: &AppState, instance_id: &str) -> Vec<SavedServer> {
    let path = state.dirs.game_dir(instance_id).join("servers.dat");
    let Ok(bytes) = std::fs::read(&path) else {
        return Vec::new();
    };
    parse_servers_dat(&bytes).unwrap_or_default()
}

fn parse_servers_dat(bytes: &[u8]) -> Option<Vec<SavedServer>> {
    let mut cur = Cur::new(bytes);
    // Root tag: TAG_Compound (id 10), followed by its (empty) name then payload.
    if cur.u8().ok()? != 10 {
        return None;
    }
    cur.nbt_string().ok()?; // root name, discarded
    let root = cur.compound().ok()?;
    let servers = root.iter().find(|(k, _)| k == "servers").map(|(_, v)| v)?;
    let Nbt::List(items) = servers else {
        return None;
    };

    let mut out = Vec::new();
    for item in items {
        let Nbt::Compound(fields) = item else { continue };
        let get = |key: &str| fields.iter().find(|(k, _)| k == key).map(|(_, v)| v);
        let str_of = |v: Option<&Nbt>| match v {
            Some(Nbt::String(s)) => s.clone(),
            _ => String::new(),
        };
        let ip = str_of(get("ip"));
        if ip.is_empty() {
            continue;
        }
        let icon = match get("icon") {
            Some(Nbt::String(s)) if !s.is_empty() => Some(s.clone()),
            _ => None,
        };
        out.push(SavedServer {
            name: str_of(get("name")),
            ip,
            icon,
        });
    }
    Some(out)
}

/// A minimal NBT value model — just enough to walk `servers.dat`. We keep every
/// tag variant so unknown fields in a server entry are skipped correctly.
#[allow(dead_code)]
enum Nbt {
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    ByteArray(Vec<u8>),
    String(String),
    List(Vec<Nbt>),
    Compound(Vec<(String, Nbt)>),
    IntArray(Vec<i32>),
    LongArray(Vec<i64>),
}

/// A cursor over a byte buffer used both for NBT (big-endian) and for parsing
/// Server List Ping response packets (VarInt-framed).
struct Cur<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Cur<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let buf: &'a [u8] = self.buf;
        let end = self
            .pos
            .checked_add(n)
            .filter(|e| *e <= buf.len())
            .ok_or_else(|| Error::Other("unexpected end of data".into()))?;
        let s = &buf[self.pos..end];
        self.pos = end;
        Ok(s)
    }

    fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn i8(&mut self) -> Result<i8> {
        Ok(self.u8()? as i8)
    }
    fn i16(&mut self) -> Result<i16> {
        let b = self.take(2)?;
        Ok(i16::from_be_bytes([b[0], b[1]]))
    }
    fn u16(&mut self) -> Result<u16> {
        let b = self.take(2)?;
        Ok(u16::from_be_bytes([b[0], b[1]]))
    }
    fn i32(&mut self) -> Result<i32> {
        let b = self.take(4)?;
        Ok(i32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn i64(&mut self) -> Result<i64> {
        let b = self.take(8)?;
        Ok(i64::from_be_bytes([
            b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
        ]))
    }
    fn f32(&mut self) -> Result<f32> {
        let b = self.take(4)?;
        Ok(f32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn f64(&mut self) -> Result<f64> {
        let b = self.take(8)?;
        Ok(f64::from_be_bytes([
            b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
        ]))
    }

    fn nbt_string(&mut self) -> Result<String> {
        let len = self.u16()? as usize;
        let b = self.take(len)?;
        Ok(String::from_utf8_lossy(b).into_owned())
    }

    fn payload(&mut self, ty: u8) -> Result<Nbt> {
        Ok(match ty {
            1 => Nbt::Byte(self.i8()?),
            2 => Nbt::Short(self.i16()?),
            3 => Nbt::Int(self.i32()?),
            4 => Nbt::Long(self.i64()?),
            5 => Nbt::Float(self.f32()?),
            6 => Nbt::Double(self.f64()?),
            7 => {
                let n = self.i32()?.max(0) as usize;
                Nbt::ByteArray(self.take(n)?.to_vec())
            }
            8 => Nbt::String(self.nbt_string()?),
            9 => {
                let elem = self.u8()?;
                let n = self.i32()?.max(0) as usize;
                let mut v = Vec::with_capacity(n.min(4096));
                for _ in 0..n {
                    v.push(self.payload(elem)?);
                }
                Nbt::List(v)
            }
            10 => Nbt::Compound(self.compound()?),
            11 => {
                let n = self.i32()?.max(0) as usize;
                let mut v = Vec::with_capacity(n.min(4096));
                for _ in 0..n {
                    v.push(self.i32()?);
                }
                Nbt::IntArray(v)
            }
            12 => {
                let n = self.i32()?.max(0) as usize;
                let mut v = Vec::with_capacity(n.min(4096));
                for _ in 0..n {
                    v.push(self.i64()?);
                }
                Nbt::LongArray(v)
            }
            other => return Err(Error::Other(format!("unknown NBT tag {other}"))),
        })
    }

    fn compound(&mut self) -> Result<Vec<(String, Nbt)>> {
        let mut out = Vec::new();
        loop {
            let ty = self.u8()?;
            if ty == 0 {
                break; // TAG_End
            }
            let name = self.nbt_string()?;
            out.push((name, self.payload(ty)?));
        }
        Ok(out)
    }

    /// Read a Minecraft protocol VarInt (LEB128, max 5 bytes).
    fn varint(&mut self) -> Result<i32> {
        let mut result: u32 = 0;
        for i in 0..5 {
            let byte = self.u8()?;
            result |= ((byte & 0x7F) as u32) << (7 * i);
            if byte & 0x80 == 0 {
                return Ok(result as i32);
            }
        }
        Err(Error::Other("VarInt too long".into()))
    }
}

// ---------------------------------------------------------------------------
// Server List Ping
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerStatus {
    pub online: bool,
    pub latency_ms: Option<u32>,
    pub players_online: Option<i64>,
    pub players_max: Option<i64>,
    pub version: Option<String>,
    pub motd: Option<String>,
    /// Data-URI PNG favicon as returned by the server, if it sends one.
    pub favicon: Option<String>,
}

/// Ping a server address (`host` or `host:port`). Never errors — an unreachable
/// server simply comes back with `online: false` so the UI can show it greyed.
pub async fn ping_server(address: &str) -> ServerStatus {
    ping_inner(address).await.unwrap_or(ServerStatus {
        online: false,
        ..Default::default()
    })
}

fn split_address(address: &str) -> (String, u16) {
    let address = address.trim();
    if let Some((host, port)) = address.rsplit_once(':') {
        if let Ok(p) = port.parse::<u16>() {
            return (host.to_string(), p);
        }
    }
    (address.to_string(), 25565)
}

async fn ping_inner(address: &str) -> Result<ServerStatus> {
    let (host, port) = split_address(address);

    let mut stream = timeout(Duration::from_secs(4), TcpStream::connect((host.as_str(), port)))
        .await
        .map_err(|_| Error::Other("connection timed out".into()))??;

    let started = Instant::now();

    // Handshake — next state 1 (status). Protocol version -1 is the conventional
    // "I'm only pinging" value that every vanilla server accepts.
    let mut handshake = Vec::new();
    write_varint(&mut handshake, 0x00);
    write_varint(&mut handshake, -1);
    write_varint(&mut handshake, host.len() as i32);
    handshake.extend_from_slice(host.as_bytes());
    handshake.extend_from_slice(&port.to_be_bytes());
    write_varint(&mut handshake, 1);
    write_frame(&mut stream, &handshake).await?;

    // Empty status request.
    write_frame(&mut stream, &[0x00]).await?;

    let packet = timeout(Duration::from_secs(5), read_frame(&mut stream))
        .await
        .map_err(|_| Error::Other("ping timed out".into()))??;
    let latency = started.elapsed().as_millis().min(u32::MAX as u128) as u32;

    let mut c = Cur::new(&packet);
    let _packet_id = c.varint()?;
    let json_len = c.varint()?.max(0) as usize;
    let json_bytes = c.take(json_len)?;
    let v: serde_json::Value = serde_json::from_slice(json_bytes)?;

    let players = v.get("players");
    let players_online = players
        .and_then(|p| p.get("online"))
        .and_then(|x| x.as_i64());
    let players_max = players.and_then(|p| p.get("max")).and_then(|x| x.as_i64());
    let version = v
        .get("version")
        .and_then(|x| x.get("name"))
        .and_then(|x| x.as_str())
        .map(strip_legacy_codes)
        .filter(|s| !s.is_empty());
    let motd = v
        .get("description")
        .map(chat_to_text)
        .map(|s| strip_legacy_codes(&s))
        .filter(|s| !s.is_empty());
    let favicon = v
        .get("favicon")
        .and_then(|x| x.as_str())
        .filter(|s| s.starts_with("data:image"))
        .map(|s| s.to_string());

    Ok(ServerStatus {
        online: true,
        latency_ms: Some(latency),
        players_online,
        players_max,
        version,
        motd,
        favicon,
    })
}

fn write_varint(buf: &mut Vec<u8>, value: i32) {
    let mut v = value as u32;
    loop {
        let mut byte = (v & 0x7F) as u8;
        v >>= 7;
        if v != 0 {
            byte |= 0x80;
        }
        buf.push(byte);
        if v == 0 {
            break;
        }
    }
}

async fn write_frame(stream: &mut TcpStream, body: &[u8]) -> Result<()> {
    let mut framed = Vec::with_capacity(body.len() + 5);
    write_varint(&mut framed, body.len() as i32);
    framed.extend_from_slice(body);
    stream.write_all(&framed).await?;
    stream.flush().await?;
    Ok(())
}

async fn read_frame(stream: &mut TcpStream) -> Result<Vec<u8>> {
    let len = read_varint_stream(stream).await? as usize;
    if len == 0 || len > 5_000_000 {
        return Err(Error::Other("invalid packet length".into()));
    }
    let mut buf = vec![0u8; len];
    stream.read_exact(&mut buf).await?;
    Ok(buf)
}

async fn read_varint_stream(stream: &mut TcpStream) -> Result<i32> {
    let mut result: u32 = 0;
    for i in 0..5 {
        let mut b = [0u8; 1];
        stream.read_exact(&mut b).await?;
        result |= ((b[0] & 0x7F) as u32) << (7 * i);
        if b[0] & 0x80 == 0 {
            return Ok(result as i32);
        }
    }
    Err(Error::Other("VarInt too long".into()))
}

/// Flatten a chat-component MOTD (string, `{text, extra}` object, or array) to
/// plain text.
fn chat_to_text(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Object(map) => {
            let mut out = String::new();
            if let Some(serde_json::Value::String(s)) = map.get("text") {
                out.push_str(s);
            }
            if let Some(serde_json::Value::Array(extra)) = map.get("extra") {
                for e in extra {
                    out.push_str(&chat_to_text(e));
                }
            }
            out
        }
        serde_json::Value::Array(arr) => arr.iter().map(chat_to_text).collect(),
        _ => String::new(),
    }
}

/// Strip legacy `§x` colour/format codes and collapse whitespace runs.
fn strip_legacy_codes(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '§' {
            chars.next();
        } else {
            out.push(c);
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}
