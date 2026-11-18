//! 网络小工具：TCP 连通性（ping）、telnet 探测、DNS 解析、IP 归属地。
//!
//! 不依赖任何异步运行时，全部是阻塞调用，调用方负责放到后台线程里跑。
//! ping 走的是 TCP 握手（无需 root 权限，也不受 ICMP 被防火墙丢弃的影响）；
//! 归属地查询用 ip2region 的 xdb 库文件，传入内存映射好的字节即可。

use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::{IktError, Result};

const MAX_READ: usize = 256 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PingSpec {
    pub host: String,
    pub port: u16,
    pub count: u32,
    pub timeout_ms: u64,
}

impl Default for PingSpec {
    fn default() -> Self {
        Self {
            host: String::new(),
            port: 443,
            count: 4,
            timeout_ms: 2000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PingSample {
    pub seq: u32,
    pub ms: Option<f64>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PingOutcome {
    pub host: String,
    pub port: u16,
    pub addrs: Vec<String>,
    pub samples: Vec<PingSample>,
    pub sent: u32,
    pub received: u32,
    pub loss_pct: f64,
    pub min_ms: f64,
    pub avg_ms: f64,
    pub max_ms: f64,
    pub elapsed_us: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeSpec {
    pub host: String,
    pub port: u16,
    pub payload: String,
    pub append_newline: bool,
    pub timeout_ms: u64,
    pub read_ms: u64,
}

impl Default for ProbeSpec {
    fn default() -> Self {
        Self {
            host: String::new(),
            port: 23,
            payload: String::new(),
            append_newline: true,
            timeout_ms: 3000,
            read_ms: 1200,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeOutcome {
    pub host: String,
    pub port: u16,
    pub addr: String,
    pub connect_ms: f64,
    pub sent_bytes: u64,
    pub recv_bytes: u64,
    pub text: String,
    pub hex: String,
    /// 对端主动关连接（读到 EOF）时为 true。
    pub closed: bool,
    pub elapsed_us: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddrInfo {
    pub ip: String,
    pub family: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DnsOutcome {
    pub host: String,
    pub addrs: Vec<AddrInfo>,
    pub elapsed_us: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeoOutcome {
    pub ip: String,
    pub version: String,
    /// 原始串，形如 `中国|0|广东省|深圳市|电信`
    pub raw: String,
    pub country: String,
    pub region: String,
    pub province: String,
    pub city: String,
    pub isp: String,
    pub elapsed_us: u64,
}

fn clean_host(host: &str) -> Result<&str> {
    let h = host.trim();
    if h.is_empty() {
        return Err(IktError::Invalid("域名 / IP 不能为空".into()));
    }
    if h.chars().any(|c| c.is_whitespace()) {
        return Err(IktError::Invalid("域名里不能有空格".into()));
    }
    Ok(h)
}

fn resolve(host: &str, port: u16) -> Result<Vec<SocketAddr>> {
    let mut addrs: Vec<SocketAddr> = (host, port)
        .to_socket_addrs()
        .map_err(|e| IktError::Invalid(format!("解析失败：{e}")))?
        .collect();
    addrs.dedup();
    if addrs.is_empty() {
        return Err(IktError::Invalid("没有解析到任何地址".into()));
    }
    Ok(addrs)
}

pub fn tcp_ping(spec: &PingSpec) -> Result<PingOutcome> {
    let started = Instant::now();
    let host = clean_host(&spec.host)?;
    let count = spec.count.clamp(1, 20);
    let timeout = Duration::from_millis(spec.timeout_ms.clamp(50, 30_000));
    let addrs = resolve(host, spec.port)?;
    let target = addrs[0];

    let mut samples = Vec::with_capacity(count as usize);
    let mut latencies: Vec<f64> = Vec::new();
    for seq in 1..=count {
        let t0 = Instant::now();
        match TcpStream::connect_timeout(&target, timeout) {
            Ok(stream) => {
                let ms = t0.elapsed().as_secs_f64() * 1000.0;
                latencies.push(ms);
                drop(stream);
                samples.push(PingSample {
                    seq,
                    ms: Some((ms * 100.0).round() / 100.0),
                    error: None,
                });
            }
            Err(e) => samples.push(PingSample {
                seq,
                ms: None,
                error: Some(short_err(&e)),
            }),
        }
    }

    let received = latencies.len() as u32;
    let (min_ms, max_ms, avg_ms) = if latencies.is_empty() {
        (0.0, 0.0, 0.0)
    } else {
        let min = latencies.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = latencies.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let avg = latencies.iter().sum::<f64>() / latencies.len() as f64;
        (round2(min), round2(avg), round2(max))
    };

    Ok(PingOutcome {
        host: host.to_string(),
        port: spec.port,
        addrs: addrs.iter().map(|a| a.ip().to_string()).collect(),
        samples,
        sent: count,
        received,
        loss_pct: round2((count - received) as f64 * 100.0 / count as f64),
        min_ms,
        avg_ms,
        max_ms,
        elapsed_us: started.elapsed().as_micros() as u64,
    })
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

fn short_err(e: &std::io::Error) -> String {
    match e.kind() {
        std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock => "超时".into(),
        std::io::ErrorKind::ConnectionRefused => "连接被拒绝".into(),
        std::io::ErrorKind::ConnectionReset => "连接被重置".into(),
        std::io::ErrorKind::HostUnreachable | std::io::ErrorKind::NetworkUnreachable => {
            "网络不可达".into()
        }
        _ => e.to_string(),
    }
}

/// 十六进制视图，每行 16 字节，右侧附 ASCII。
fn hex_dump(bytes: &[u8], limit: usize) -> String {
    let slice = &bytes[..bytes.len().min(limit)];
    let mut out = String::new();
    for (i, chunk) in slice.chunks(16).enumerate() {
        let mut hex_part = String::new();
        let mut ascii_part = String::new();
        for b in chunk {
            hex_part.push_str(&format!("{b:02x} "));
            ascii_part.push(if (0x20..0x7f).contains(b) { *b as char } else { '.' });
        }
        while hex_part.len() < 48 {
            hex_part.push(' ');
        }
        out.push_str(&format!("{:04x}  {hex_part} {ascii_part}\n", i * 16));
    }
    out
}

pub fn tcp_probe(spec: &ProbeSpec) -> Result<ProbeOutcome> {
    let started = Instant::now();
    let host = clean_host(&spec.host)?;
    let timeout = Duration::from_millis(spec.timeout_ms.clamp(50, 30_000));
    let read_ms = Duration::from_millis(spec.read_ms.clamp(50, 30_000));
    let addrs = resolve(host, spec.port)?;
    let target = addrs[0];

    let t0 = Instant::now();
    let mut stream = TcpStream::connect_timeout(&target, timeout)
        .map_err(|e| IktError::Invalid(format!("连不上 {target}：{}", short_err(&e))))?;
    let connect_ms = round2(t0.elapsed().as_secs_f64() * 1000.0);

    let mut payload_bytes = spec.payload.clone().into_bytes();
    if spec.append_newline && !payload_bytes.is_empty() {
        payload_bytes.extend_from_slice(b"\r\n");
    }
    let sent_bytes = payload_bytes.len() as u64;
    if !payload_bytes.is_empty() {
        stream
            .write_all(&payload_bytes)
            .map_err(|e| IktError::Invalid(format!("发送失败：{e}")))?;
        let _ = stream.flush();
    }

    stream
        .set_read_timeout(Some(read_ms))
        .map_err(|e| IktError::Invalid(format!("设置读超时失败：{e}")))?;

    let mut buf = Vec::new();
    let mut chunk = [0u8; 8192];
    let mut closed = false;
    let deadline = Instant::now() + read_ms + Duration::from_millis(50);
    loop {
        if buf.len() >= MAX_READ {
            break;
        }
        match stream.read(&mut chunk) {
            Ok(0) => {
                closed = true;
                break;
            }
            Ok(n) => {
                buf.extend_from_slice(&chunk[..n]);
                // 收到一段就再等一轮，直到对端安静下来或者超时
                if Instant::now() >= deadline {
                    break;
                }
            }
            Err(e) => match e.kind() {
                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut => break,
                std::io::ErrorKind::ConnectionReset => {
                    closed = true;
                    break;
                }
                _ => return Err(IktError::Invalid(format!("读取失败：{e}"))),
            },
        }
    }

    let text = String::from_utf8_lossy(&buf)
        .replace("\r\n", "\n")
        .replace('\r', "\n");
    Ok(ProbeOutcome {
        host: host.to_string(),
        port: spec.port,
        addr: target.to_string(),
        connect_ms,
        sent_bytes,
        recv_bytes: buf.len() as u64,
        hex: hex_dump(&buf, 4096),
        text,
        closed,
        elapsed_us: started.elapsed().as_micros() as u64,
    })
}

pub fn dns_lookup(host: &str) -> Result<DnsOutcome> {
    let started = Instant::now();
    let host = clean_host(host)?;
    let addrs = resolve(host, 0)?;
    let mut seen: Vec<IpAddr> = Vec::new();
    let mut out = Vec::new();
    for a in addrs {
        let ip = a.ip();
        if seen.contains(&ip) {
            continue;
        }
        seen.push(ip);
        out.push(AddrInfo {
            ip: ip.to_string(),
            family: if ip.is_ipv4() { "IPv4" } else { "IPv6" }.into(),
        });
    }
    out.sort_by(|a, b| a.family.cmp(&b.family).then(a.ip.cmp(&b.ip)));
    Ok(DnsOutcome {
        host: host.to_string(),
        addrs: out,
        elapsed_us: started.elapsed().as_micros() as u64,
    })
}

/// 把 `中国|0|广东省|深圳市|电信` 拆成字段。
fn split_region(raw: &str) -> (String, String, String, String, String) {
    let mut parts = raw.split('|').map(|s| s.trim());
    let country = parts.next().unwrap_or("");
    let region = parts.next().unwrap_or("");
    let province = parts.next().unwrap_or("");
    let city = parts.next().unwrap_or("");
    let isp = parts.next().unwrap_or("");
    let norm = |s: &str| if s == "0" { String::new() } else { s.to_string() };
    (
        norm(country),
        norm(region),
        norm(province),
        norm(city),
        norm(isp),
    )
}

/// xdb 头部长度 + 二级向量索引长度。
const XDB_HEADER: usize = 256;
const XDB_VECTOR: usize = 256 * 256 * 8;

/// xdb-parse 内部全是裸切片、不做边界检查，脏数据会直接 panic。
/// 查询前自己把头部字段校验一遍，保证 base/end 落在缓冲区里。
fn check_db(db: &[u8]) -> Result<()> {
    let bad = || IktError::Invalid("归属地库文件损坏或格式不对".into());
    if db.len() < XDB_HEADER + XDB_VECTOR {
        return Err(bad());
    }
    let version = u16::from_ne_bytes([db[0], db[1]]);
    if !(1..=3).contains(&version) {
        return Err(bad());
    }
    let base = u32::from_ne_bytes([db[8], db[9], db[10], db[11]]) as usize;
    let end = u32::from_ne_bytes([db[12], db[13], db[14], db[15]]) as usize;
    if base < XDB_HEADER + XDB_VECTOR || end <= base || end >= db.len() {
        return Err(bad());
    }
    Ok(())
}

/// 兜底：即使校验漏了某种畸形情况，也不要让 panic 冒到命令层。
fn search_region(ip: &IpAddr, db: &[u8]) -> Result<String> {
    check_db(db)?;
    let text = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        xdb_parse::search_ip(&ip.to_string(), db).map(|s| s.to_string())
    }))
    .map_err(|_| IktError::Invalid("归属地查询失败：库文件读取越界".into()))?;
    text.map_err(|e| IktError::Invalid(format!("归属地查询失败：{e}")))
}

/// `target` 可以是 IP，也可以是域名（先解析成 IP 再查）。
pub fn geo_lookup(target: &str, v4_db: Option<&[u8]>, v6_db: Option<&[u8]>) -> Result<GeoOutcome> {
    let started = Instant::now();
    let raw_target = clean_host(target)?;
    let ip: IpAddr = match raw_target.parse::<IpAddr>() {
        Ok(ip) => ip,
        Err(_) => {
            let addrs = resolve(raw_target, 0)?;
            let preferred = addrs
                .iter()
                .find(|a| a.is_ipv4())
                .unwrap_or(&addrs[0]);
            preferred.ip()
        }
    };

    let (db, version) = if ip.is_ipv4() {
        (
            v4_db.ok_or_else(|| IktError::Invalid("缺少 IPv4 归属地库（ip2region_v4.xdb）".into()))?,
            "IPv4",
        )
    } else {
        (
            v6_db.ok_or_else(|| IktError::Invalid("未内置 IPv6 归属地库".into()))?,
            "IPv6",
        )
    };

    let region = search_region(&ip, db)?;
    let (country, reg, province, city, isp) = split_region(&region);
    Ok(GeoOutcome {
        ip: ip.to_string(),
        version: version.into(),
        raw: region,
        country,
        region: reg,
        province,
        city,
        isp,
        elapsed_us: started.elapsed().as_micros() as u64,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn region_split() {
        let (c, r, p, city, isp) = split_region("中国|0|广东省|深圳市|电信");
        assert_eq!((c.as_str(), r.as_str(), p.as_str(), city.as_str(), isp.as_str()), ("中国", "", "广东省", "深圳市", "电信"));
    }

    #[test]
    fn region_split_short() {
        let (c, r, p, city, isp) = split_region("局域网|0|0|0|0");
        assert_eq!(c, "局域网");
        assert!(r.is_empty() && p.is_empty() && city.is_empty() && isp.is_empty());
        let (c2, ..) = split_region("美国");
        assert_eq!(c2, "美国");
    }

    #[test]
    fn host_validation() {
        assert!(clean_host("").is_err());
        assert!(clean_host("  ").is_err());
        assert!(clean_host("a b.com").is_err());
        assert_eq!(clean_host(" example.com ").unwrap(), "example.com");
        assert!(dns_lookup("  ").is_err());
    }

    #[test]
    fn geo_requires_db() {
        let err = geo_lookup("1.1.1.1", None, None).unwrap_err();
        assert!(err.to_string().contains("缺少 IPv4 归属地库"), "{err}");
    }

    #[test]
    fn geo_with_garbage_db() {
        let err = geo_lookup("1.1.1.1", Some(&[0u8; 32]), None).unwrap_err();
        assert!(err.to_string().contains("归属地库文件损坏"), "{err}");
    }

    #[test]
    fn geo_real_db() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../src-tauri/ip2region_v4.xdb");
        let Ok(db) = std::fs::read(&path) else {
            return;
        };
        let out = geo_lookup("114.114.114.114", Some(&db), None).unwrap();
        assert_eq!(out.version, "IPv4");
        assert_eq!(out.country, "中国");
        assert!(!out.raw.is_empty());
    }

    #[test]
    fn hex_dump_shape() {
        let dump = hex_dump(b"Hello, world!\n", 4096);
        assert!(dump.starts_with("0000  48 65 6c 6c"), "{dump}");
        assert!(dump.contains("Hello, world!."), "{dump}");
    }
}
