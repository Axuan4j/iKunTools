//! 随机标识 / 随机文本生成。
//!
//! 覆盖 UUID v4、UUID v7、ULID、NanoID、随机密码、随机字符串、随机字节、随机整数和「从清单里抽 N 条」。
//! 所有随机源都走 `rand::thread_rng()`（操作系统熵池）。

use std::time::{Instant, SystemTime, UNIX_EPOCH};

use rand::{seq::SliceRandom, Rng};
use serde::{Deserialize, Serialize};

use crate::{IktError, Result};

pub const KINDS: [&str; 9] = [
    "uuid4", "uuid7", "ulid", "nanoid", "password", "text", "bytes", "int", "pick",
];

/// NanoID 官方默认字符集（64 个 URL 安全字符）。
const NANOID_ALPHABET: &str = "useandom-26T198340PX75pxJACKVERYMINDBUSHWOLF_GQZbfghjklqvwyzrict";
const CROCKFORD: &[u8] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
const LOWER: &str = "abcdefghijklmnopqrstuvwxyz";
const UPPER: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
const DIGITS: &str = "0123456789";
const SYMBOLS: &str = "!@#$%^&*()-_=+[]{};:,.?/";
const AMBIGUOUS: &str = "0O1lI|`'\"";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RandomSpec {
    /// `uuid4` / `uuid7` / `ulid` / `nanoid` / `password` / `text` / `bytes` / `int` / `pick`
    pub kind: String,
    pub count: u32,
    pub length: u32,
    pub alphabet: String,
    pub digits: bool,
    pub lower: bool,
    pub upper: bool,
    pub symbols: bool,
    pub exclude_ambiguous: bool,
    pub bytes_len: u32,
    pub bytes_encoding: String,
    pub min: i64,
    pub max: i64,
    pub pool: String,
}

impl Default for RandomSpec {
    fn default() -> Self {
        Self {
            kind: "uuid4".into(),
            count: 1,
            length: 16,
            alphabet: String::new(),
            digits: true,
            lower: true,
            upper: true,
            symbols: false,
            exclude_ambiguous: true,
            bytes_len: 16,
            bytes_encoding: "hex".into(),
            min: 1,
            max: 100,
            pool: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RandomOutcome {
    pub items: Vec<String>,
    pub output: String,
    pub count: u32,
    /// 每条的理论熵（bit），无法估算时为 0。
    pub bits: f64,
    /// 字符集大小，只有文本类才有。
    pub alphabet_size: u32,
    pub elapsed_us: u64,
}

fn hex_lower(bytes: &[u8]) -> String {
    hex::encode(bytes)
}

fn uuid_v4(rng: &mut impl Rng) -> String {
    let mut b = [0u8; 16];
    rng.fill(&mut b);
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let h = hex_lower(&b);
    format!(
        "{}-{}-{}-{}-{}",
        &h[0..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..32]
    )
}

fn uuid_v7(rng: &mut impl Rng) -> String {
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let rand_a: u16 = rng.gen();
    let rand_b: u64 = rng.gen();
    let mut b = [0u8; 16];
    for (i, slot) in b.iter_mut().take(6).enumerate() {
        *slot = (ms >> (8 * (5 - i))) as u8;
    }
    b[6] = 0x70 | ((rand_a >> 8) as u8 & 0x0f);
    b[7] = (rand_a & 0xff) as u8;
    b[8] = 0x80 | ((rand_b >> 56) as u8 & 0x3f);
    for i in 0..7 {
        b[9 + i] = (rand_b >> (48 - 8 * i)) as u8;
    }
    let h = hex_lower(&b);
    format!(
        "{}-{}-{}-{}-{}",
        &h[0..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..32]
    )
}

fn ulid(rng: &mut impl Rng) -> String {
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let mut bytes = [0u8; 16];
    bytes[0] = (ms >> 40) as u8;
    bytes[1] = (ms >> 32) as u8;
    bytes[2] = (ms >> 24) as u8;
    bytes[3] = (ms >> 16) as u8;
    bytes[4] = (ms >> 8) as u8;
    bytes[5] = ms as u8;
    rng.fill(&mut bytes[6..]);
    // 48 bit 时间戳 + 80 bit 随机 → 26 个 5 bit 字符（最高字符只有 3 位有效）
    let ts = ms & 0xffff_ffff_ffff;
    let rand: u128 = ((rng.gen::<u64>() as u128) << 16) | rng.gen::<u16>() as u128;
    let value: u128 = ((ts as u128) << 80) | (rand & ((1u128 << 80) - 1));
    let mut out = String::with_capacity(26);
    for i in (0..26).rev() {
        out.push(CROCKFORD[((value >> (5 * i)) & 0x1f) as usize] as char);
    }
    out
}

fn nanoid(rng: &mut impl Rng, len: usize) -> String {
    let chars: Vec<char> = NANOID_ALPHABET.chars().collect();
    (0..len)
        .map(|_| chars[rng.gen_range(0..chars.len())])
        .collect()
}

fn unique_chars(s: &str) -> Vec<char> {
    let mut seen = Vec::new();
    for c in s.chars() {
        if !seen.contains(&c) {
            seen.push(c);
        }
    }
    seen
}

fn password_alphabet(spec: &RandomSpec) -> Result<Vec<char>> {
    let mut pool = String::new();
    if spec.lower {
        pool.push_str(LOWER);
    }
    if spec.upper {
        pool.push_str(UPPER);
    }
    if spec.digits {
        pool.push_str(DIGITS);
    }
    if spec.symbols {
        pool.push_str(SYMBOLS);
    }
    if pool.is_empty() {
        return Err(IktError::Invalid("至少要勾选一类字符".into()));
    }
    let mut chars: Vec<char> = unique_chars(&pool);
    if spec.exclude_ambiguous {
        chars.retain(|c| !AMBIGUOUS.contains(*c));
    }
    Ok(chars)
}

fn password(rng: &mut impl Rng, spec: &RandomSpec, len: usize) -> Result<String> {
    let alphabet = password_alphabet(spec)?;
    let mut required: Vec<char> = Vec::new();
    let classes: [(&str, bool); 4] = [
        (LOWER, spec.lower),
        (UPPER, spec.upper),
        (DIGITS, spec.digits),
        (SYMBOLS, spec.symbols),
    ];
    for (class, on) in classes {
        if !on {
            continue;
        }
        let filtered: Vec<char> = class
            .chars()
            .filter(|c| !spec.exclude_ambiguous || !AMBIGUOUS.contains(*c))
            .collect();
        if !filtered.is_empty() {
            required.push(filtered[rng.gen_range(0..filtered.len())]);
        }
    }
    if len < required.len() {
        return Err(IktError::Invalid(format!(
            "长度至少要 {} 位才能覆盖勾选的字符类型",
            required.len()
        )));
    }
    let mut chars: Vec<char> = (0..len - required.len())
        .map(|_| alphabet[rng.gen_range(0..alphabet.len())])
        .collect();
    chars.extend(required);
    chars.shuffle(rng);
    Ok(chars.into_iter().collect())
}

fn text(rng: &mut impl Rng, spec: &RandomSpec, len: usize) -> Result<(String, usize)> {
    let alphabet: Vec<char> = if spec.alphabet.trim().is_empty() {
        unique_chars(&format!("{LOWER}{UPPER}{DIGITS}"))
    } else {
        unique_chars(spec.alphabet.trim())
    };
    if alphabet.is_empty() {
        return Err(IktError::Invalid("字符集为空".into()));
    }
    let s: String = (0..len)
        .map(|_| alphabet[rng.gen_range(0..alphabet.len())])
        .collect();
    Ok((s, alphabet.len()))
}

fn bytes(rng: &mut impl Rng, len: usize, encoding: &str) -> Result<String> {
    if len == 0 {
        return Err(IktError::Invalid("字节长度要大于 0".into()));
    }
    let mut buf = vec![0u8; len];
    rng.fill(&mut buf[..]);
    match encoding {
        "hex" => Ok(hex::encode(&buf)),
        "base64" => {
            use base64::{engine::general_purpose::STANDARD, Engine};
            Ok(STANDARD.encode(&buf))
        }
        other => Err(IktError::Invalid(format!("不支持的输出编码：{other}"))),
    }
}

fn int(rng: &mut impl Rng, min: i64, max: i64) -> Result<i64> {
    if min > max {
        return Err(IktError::Invalid("最小值不能大于最大值".into()));
    }
    if min == max {
        return Ok(min);
    }
    Ok(rng.gen_range(min..=max))
}

fn pick(rng: &mut impl Rng, pool: &str, count: usize) -> Result<Vec<String>> {
    let lines: Vec<&str> = pool
        .lines()
        .map(str::trim_end)
        .filter(|l| !l.trim().is_empty())
        .collect();
    if lines.is_empty() {
        return Err(IktError::Invalid("候选清单为空".into()));
    }
    let mut owned: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
    owned.shuffle(rng);
    owned.truncate(count.min(owned.len()));
    Ok(owned)
}

pub fn generate(spec: &RandomSpec) -> Result<RandomOutcome> {
    let started = Instant::now();
    let kind = spec.kind.trim();
    if !KINDS.contains(&kind) {
        return Err(IktError::Invalid(format!("不认识的类型：{kind}")));
    }
    let count = spec.count.clamp(1, 1000) as usize;
    let length = spec.length.clamp(1, 4096) as usize;
    let mut rng = rand::thread_rng();

    let mut bits = 0.0f64;
    let mut alphabet_size = 0u32;
    let mut items: Vec<String> = Vec::with_capacity(count);

    match kind {
        "uuid4" => {
            bits = 122.0;
            for _ in 0..count {
                items.push(uuid_v4(&mut rng));
            }
        }
        "uuid7" => {
            bits = 74.0;
            for _ in 0..count {
                items.push(uuid_v7(&mut rng));
            }
        }
        "ulid" => {
            bits = 80.0;
            for _ in 0..count {
                items.push(ulid(&mut rng));
            }
        }
        "nanoid" => {
            alphabet_size = NANOID_ALPHABET.chars().count() as u32;
            bits = length as f64 * (alphabet_size as f64).log2();
            for _ in 0..count {
                items.push(nanoid(&mut rng, length));
            }
        }
        "password" => {
            alphabet_size = password_alphabet(spec)?.len() as u32;
            bits = length as f64 * (alphabet_size as f64).log2();
            for _ in 0..count {
                items.push(password(&mut rng, spec, length)?);
            }
        }
        "text" => {
            let probe = text(&mut rng, spec, length)?;
            alphabet_size = probe.1 as u32;
            bits = length as f64 * (probe.1 as f64).log2();
            items.push(probe.0);
            for _ in 1..count {
                items.push(text(&mut rng, spec, length)?.0);
            }
        }
        "bytes" => {
            bits = spec.bytes_len as f64 * 8.0;
            for _ in 0..count {
                items.push(bytes(&mut rng, spec.bytes_len as usize, &spec.bytes_encoding)?);
            }
        }
        "int" => {
            let span = (spec.max as i128 - spec.min as i128 + 1).max(1);
            bits = (span as f64).log2();
            for _ in 0..count {
                items.push(int(&mut rng, spec.min, spec.max)?.to_string());
            }
        }
        _ => {
            items = pick(&mut rng, &spec.pool, count)?;
        }
    }

    let output = items.join("\n");
    Ok(RandomOutcome {
        count: items.len() as u32,
        output,
        items,
        bits,
        alphabet_size,
        elapsed_us: started.elapsed().as_micros() as u64,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(kind: &str) -> RandomSpec {
        RandomSpec {
            kind: kind.into(),
            ..Default::default()
        }
    }

    #[test]
    fn uuid4_shape() {
        let out = generate(&RandomSpec {
            count: 32,
            ..spec("uuid4")
        })
        .unwrap();
        for u in &out.items {
            assert_eq!(u.len(), 36, "{u}");
            let parts: Vec<&str> = u.split('-').collect();
            assert_eq!(
                parts.iter().map(|p| p.len()).collect::<Vec<_>>(),
                vec![8, 4, 4, 4, 12]
            );
            assert!(u.chars().all(|c| c.is_ascii_hexdigit() || c == '-'));
            assert_eq!(&u[14..15], "4", "version 位不对：{u}");
            assert!(matches!(&u[19..20], "8" | "9" | "a" | "b"), "variant 位不对：{u}");
        }
    }

    #[test]
    fn uuid7_has_version_and_variant() {
        let out = generate(&RandomSpec {
            count: 16,
            ..spec("uuid7")
        })
        .unwrap();
        for u in &out.items {
            assert_eq!(u.len(), 36);
            assert_eq!(&u[14..15], "7", "{u}");
            assert!(matches!(&u[19..20], "8" | "9" | "a" | "b"), "{u}");
        }
    }

    #[test]
    fn ulid_shape() {
        let out = generate(&RandomSpec {
            count: 16,
            ..spec("ulid")
        })
        .unwrap();
        for u in &out.items {
            assert_eq!(u.len(), 26, "{u}");
            assert!(u.chars().all(|c| CROCKFORD.contains(&(c as u8))), "{u}");
        }
        // 时间前缀应当一致（同一毫秒内）
        let prefix: String = out.items[0].chars().take(8).collect();
        assert!(prefix.chars().all(|c| CROCKFORD.contains(&(c as u8))));
    }

    #[test]
    fn ulid_time_prefix_is_current() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        let out = generate(&spec("ulid")).unwrap();
        let head: Vec<char> = out.items[0].chars().take(10).collect();
        let mut value: u64 = 0;
        for c in head {
            let idx = CROCKFORD.iter().position(|&x| x == c as u8).unwrap() as u64;
            value = value * 32 + idx;
        }
        let delta = (value as i128 - now as i128).abs();
        assert!(delta < 5_000, "时间前缀偏差过大：{delta} ms");
    }

    #[test]
    fn nanoid_length_and_alphabet() {
        let out = generate(&RandomSpec {
            count: 8,
            length: 21,
            ..spec("nanoid")
        })
        .unwrap();
        for s in &out.items {
            assert_eq!(s.chars().count(), 21);
            assert!(s.chars().all(|c| NANOID_ALPHABET.contains(c)), "{s}");
        }
        assert_eq!(out.alphabet_size, 64);
    }

    #[test]
    fn password_covers_selected_classes() {
        let out = generate(&RandomSpec {
            count: 64,
            length: 20,
            digits: true,
            lower: true,
            upper: true,
            symbols: true,
            exclude_ambiguous: true,
            ..spec("password")
        })
        .unwrap();
        for p in &out.items {
            assert_eq!(p.chars().count(), 20);
            assert!(p.chars().any(|c| c.is_ascii_lowercase()), "{p}");
            assert!(p.chars().any(|c| c.is_ascii_uppercase()), "{p}");
            assert!(p.chars().any(|c| c.is_ascii_digit()), "{p}");
            assert!(p.chars().any(|c| SYMBOLS.contains(c)), "{p}");
            assert!(!p.chars().any(|c| AMBIGUOUS.contains(c)), "含易混字符：{p}");
        }
    }

    #[test]
    fn password_too_short_for_classes() {
        let err = generate(&RandomSpec {
            length: 2,
            digits: true,
            lower: true,
            upper: true,
            symbols: true,
            ..spec("password")
        })
        .unwrap_err();
        assert!(err.to_string().contains("长度至少要 4"), "{err}");
    }

    #[test]
    fn password_needs_one_class() {
        let err = generate(&RandomSpec {
            digits: false,
            lower: false,
            upper: false,
            symbols: false,
            ..spec("password")
        })
        .unwrap_err();
        assert!(err.to_string().contains("至少要勾选"), "{err}");
    }

    #[test]
    fn text_custom_alphabet() {
        let out = generate(&RandomSpec {
            count: 4,
            length: 32,
            alphabet: "abc".into(),
            ..spec("text")
        })
        .unwrap();
        assert_eq!(out.alphabet_size, 3);
        for s in &out.items {
            assert_eq!(s.chars().count(), 32);
            assert!(s.chars().all(|c| "abc".contains(c)), "{s}");
        }
    }

    #[test]
    fn text_default_alphabet_is_alnum() {
        let out = generate(&RandomSpec {
            length: 64,
            ..spec("text")
        })
        .unwrap();
        assert_eq!(out.alphabet_size, 62);
        assert!(out.items[0].chars().all(|c| c.is_ascii_alphanumeric()));
    }

    #[test]
    fn bytes_hex_and_base64() {
        let hex_out = generate(&RandomSpec {
            bytes_len: 16,
            bytes_encoding: "hex".into(),
            ..spec("bytes")
        })
        .unwrap();
        assert_eq!(hex_out.items[0].len(), 32);
        let b64 = generate(&RandomSpec {
            bytes_len: 16,
            bytes_encoding: "base64".into(),
            ..spec("bytes")
        })
        .unwrap();
        assert_eq!(b64.items[0].len(), 24);
    }

    #[test]
    fn int_in_range() {
        let out = generate(&RandomSpec {
            count: 200,
            min: 5,
            max: 7,
            ..spec("int")
        })
        .unwrap();
        for s in &out.items {
            let v: i64 = s.parse().unwrap();
            assert!((5..=7).contains(&v), "{v}");
        }
    }

    #[test]
    fn int_rejects_reversed_range() {
        let err = generate(&RandomSpec {
            min: 10,
            max: 1,
            ..spec("int")
        })
        .unwrap_err();
        assert!(err.to_string().contains("最小值不能大于最大值"));
    }

    #[test]
    fn pick_without_replacement() {
        let out = generate(&RandomSpec {
            count: 3,
            pool: "a\nb\nc\nd\n\n".into(),
            ..spec("pick")
        })
        .unwrap();
        assert_eq!(out.items.len(), 3);
        let mut sorted = out.items.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), 3, "出现重复：{:?}", out.items);
        assert!(out.items.iter().all(|v| ["a", "b", "c", "d"].contains(&v.as_str())));
    }

    #[test]
    fn count_is_clamped() {
        let out = generate(&RandomSpec {
            count: 0,
            ..spec("uuid4")
        })
        .unwrap();
        assert_eq!(out.items.len(), 1);
    }

    #[test]
    fn unknown_kind_rejected() {
        assert!(generate(&spec("md5")).is_err());
    }

    #[test]
    fn contract_is_camel_case() {
        let json = serde_json::to_string(&generate(&spec("uuid4")).unwrap()).unwrap();
        for key in ["\"items\"", "\"output\"", "\"bits\"", "\"alphabetSize\"", "\"elapsedUs\""] {
            assert!(json.contains(key), "缺少 {key}：{json}");
        }
    }
}
