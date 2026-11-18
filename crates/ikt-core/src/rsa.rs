//! RSA：密钥对生成、公钥加密、私钥解密。
//!
//! 三种标准方案（OAEP-SHA256 / OAEP-SHA1 / PKCS#1 v1.5）用于和别的系统对接，
//! 外加一个 `ikt3` 混合封装：RSA 只包裹 32 字节会话密钥，正文交给 AES-256-GCM，
//! 这样长文本也能一次加密完。
//!
//! 密钥生成放在这里而不是壳层，是为了将来 CLI 能直接复用（`ikt rsa keygen`）。

use std::time::Instant;

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::engine::general_purpose::{STANDARD as B64, URL_SAFE_NO_PAD};
use base64::Engine;
use rand::{thread_rng, RngCore};
use rsa::pkcs1::{DecodeRsaPrivateKey, DecodeRsaPublicKey, EncodeRsaPrivateKey, EncodeRsaPublicKey};
use rsa::pkcs8::{DecodePrivateKey, DecodePublicKey, EncodePrivateKey, EncodePublicKey, LineEnding};
use rsa::traits::PublicKeyParts;
use rsa::{Oaep, Pkcs1v15Encrypt, RsaPrivateKey, RsaPublicKey};
use sha1::Sha1;
use sha2::{Digest, Sha256};
use zeroize::Zeroize;

use crate::{IktError, Result};

/// 支持生成的密钥长度。1024 位已经不安全，不再提供。
pub const RSA_KEY_BITS: [usize; 3] = [2048, 3072, 4096];
/// 前端下拉框复用这份顺序。
pub const RSA_SCHEMES: [&str; 4] = ["oaep-sha256", "oaep-sha1", "pkcs1v15", "hybrid"];
pub const RSA_ENCODINGS: [&str; 3] = ["auto", "base64", "hex"];

const HYBRID_PREFIX: &str = "ikt3.";
const AES_KEY_LEN: usize = 32;
const GCM_IV_LEN: usize = 12;

// ---------------------------------------------------------------------------
// 数据结构
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RsaKeyPair {
    pub public_pem: String,
    pub private_pem: String,
    pub public_pkcs1_pem: String,
    pub private_pkcs1_pem: String,
    pub bits: usize,
    pub exponent: String,
    pub fingerprint_sha256: String,
    pub fingerprint_openssh: String,
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RsaKeyInfo {
    /// public | private
    pub kind: String,
    pub bits: usize,
    pub exponent: String,
    pub fingerprint_sha256: String,
    pub fingerprint_openssh: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RsaSpec {
    /// oaep-sha256 | oaep-sha1 | pkcs1v15 | hybrid
    pub scheme: String,
    /// base64 | hex
    pub output_encoding: String,
    /// auto | base64 | hex
    pub input_encoding: String,
}

impl Default for RsaSpec {
    fn default() -> Self {
        Self {
            scheme: "oaep-sha256".into(),
            output_encoding: "base64".into(),
            input_encoding: "auto".into(),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RsaOutcome {
    pub output: String,
    pub scheme: String,
    pub key_bits: usize,
    pub fingerprint_openssh: String,
    /// 单条报文上限（字节），0 表示该方案不限长度
    pub max_bytes: usize,
    /// 本次输入 / 输出的字节数
    pub payload_bytes: usize,
    pub elapsed_ms: u64,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scheme {
    OaepSha256,
    OaepSha1,
    Pkcs1v15,
    Hybrid,
}

impl Scheme {
    fn parse(raw: &str) -> Result<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "" | "oaep-sha256" | "oaep" | "oaep256" => Ok(Scheme::OaepSha256),
            "oaep-sha1" | "oaep1" => Ok(Scheme::OaepSha1),
            "pkcs1v15" | "pkcs1" | "v1.5" => Ok(Scheme::Pkcs1v15),
            "hybrid" | "ikt3" => Ok(Scheme::Hybrid),
            other => Err(IktError::Invalid(format!(
                "未知的 RSA 方案：{other}。可用：oaep-sha256 / oaep-sha1 / pkcs1v15 / hybrid"
            ))),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Scheme::OaepSha256 => "RSA-OAEP-SHA256",
            Scheme::OaepSha1 => "RSA-OAEP-SHA1",
            Scheme::Pkcs1v15 => "RSA-PKCS#1 v1.5",
            Scheme::Hybrid => "RSA-OAEP + AES-256-GCM",
        }
    }

    /// RSA 单条报文的硬上限：k - 填充开销。None 表示不受 RSA 长度限制。
    fn max_bytes(self, k: usize) -> Option<usize> {
        match self {
            Scheme::OaepSha256 => Some(k.saturating_sub(2 * 32 + 2)),
            Scheme::OaepSha1 => Some(k.saturating_sub(2 * 20 + 2)),
            Scheme::Pkcs1v15 => Some(k.saturating_sub(11)),
            Scheme::Hybrid => None,
        }
    }
}

// ---------------------------------------------------------------------------
// 密钥生成 / 查看
// ---------------------------------------------------------------------------

pub fn generate_keypair(bits: usize) -> Result<RsaKeyPair> {
    if !RSA_KEY_BITS.contains(&bits) {
        let list = RSA_KEY_BITS
            .iter()
            .map(|b| b.to_string())
            .collect::<Vec<_>>()
            .join(" / ");
        return Err(IktError::Invalid(format!("只支持 {list} 位密钥")));
    }

    let started = Instant::now();
    let mut rng = thread_rng();
    let private = RsaPrivateKey::new(&mut rng, bits)
        .map_err(|e| IktError::Internal(format!("密钥生成失败：{e}")))?;
    let public = RsaPublicKey::from(&private);

    let public_pem = public
        .to_public_key_pem(LineEnding::LF)
        .map_err(|e| IktError::Internal(format!("公钥编码失败：{e}")))?;
    let private_pem = private
        .to_pkcs8_pem(LineEnding::LF)
        .map_err(|e| IktError::Internal(format!("私钥编码失败：{e}")))?
        .as_str()
        .to_owned();
    let public_pkcs1_pem = public
        .to_pkcs1_pem(LineEnding::LF)
        .map_err(|e| IktError::Internal(format!("公钥编码失败：{e}")))?
        .as_str()
        .to_owned();
    let private_pkcs1_pem = private
        .to_pkcs1_pem(LineEnding::LF)
        .map_err(|e| IktError::Internal(format!("私钥编码失败：{e}")))?
        .as_str()
        .to_owned();

    let (fingerprint_sha256, fingerprint_openssh) = fingerprint_of(&public)?;

    Ok(RsaKeyPair {
        public_pem,
        private_pem,
        public_pkcs1_pem,
        private_pkcs1_pem,
        bits,
        exponent: public.e().to_string(),
        fingerprint_sha256,
        fingerprint_openssh,
        elapsed_ms: started.elapsed().as_millis() as u64,
    })
}

/// 看一眼手里这串 PEM 是什么：公钥还是私钥、多少位、指纹多少。
pub fn inspect(key_pem: &str) -> Result<RsaKeyInfo> {
    let text = normalize_pem(key_pem);
    if text.is_empty() {
        return Err(IktError::Invalid("先粘贴一段 PEM".into()));
    }
    if let Ok(private) = parse_private_key(&text) {
        let public = RsaPublicKey::from(&private);
        let (fingerprint_sha256, fingerprint_openssh) = fingerprint_of(&public)?;
        return Ok(RsaKeyInfo {
            kind: "private".into(),
            bits: public.size() * 8,
            exponent: public.e().to_string(),
            fingerprint_sha256,
            fingerprint_openssh,
        });
    }
    let public = parse_public_key(&text)?;
    let (fingerprint_sha256, fingerprint_openssh) = fingerprint_of(&public)?;
    Ok(RsaKeyInfo {
        kind: "public".into(),
        bits: public.size() * 8,
        exponent: public.e().to_string(),
        fingerprint_sha256,
        fingerprint_openssh,
    })
}

// ---------------------------------------------------------------------------
// 加密 / 解密
// ---------------------------------------------------------------------------

pub fn rsa_encrypt(spec: &RsaSpec, public_pem: &str, plaintext: &str) -> Result<RsaOutcome> {
    let started = Instant::now();
    let key = parse_public_key(&normalize_pem(public_pem))?;
    let k = key.size();
    let scheme = Scheme::parse(&spec.scheme)?;
    let data = plaintext.as_bytes();

    if data.is_empty() {
        return Err(IktError::Invalid("先输入要加密的内容".into()));
    }
    if let Some(max) = scheme.max_bytes(k) {
        if data.len() > max {
            return Err(IktError::Invalid(format!(
                "{} 用 {} 位密钥最多加密 {} 字节，当前 {} 字节。长文本请改用「混合 ikt3」。",
                scheme.label(),
                k * 8,
                max,
                data.len()
            )));
        }
    }

    let mut notes = Vec::new();
    let mut rng = thread_rng();
    let output = match scheme {
        Scheme::Hybrid => {
            notes.push(format!(
                "ikt3：RSA-OAEP 只包裹 {AES_KEY_LEN} 字节会话密钥，正文用 AES-256-GCM，随机 IV 一并写进密文"
            ));
            hybrid_seal(&key, data)?
        }
        _ => {
            let encrypted = match scheme {
                Scheme::OaepSha256 => key.encrypt(&mut rng, Oaep::new::<Sha256>(), data),
                Scheme::OaepSha1 => key.encrypt(&mut rng, Oaep::new::<Sha1>(), data),
                Scheme::Pkcs1v15 => key.encrypt(&mut rng, Pkcs1v15Encrypt, data),
                Scheme::Hybrid => unreachable!(),
            }
            .map_err(|e| IktError::Invalid(format!("加密失败：{e}")))?;
            encode_bytes(&encrypted, &spec.output_encoding)
        }
    };

    if scheme == Scheme::Pkcs1v15 {
        notes.push("PKCS#1 v1.5 没有 OAEP 的随机化校验，新系统建议用 OAEP".into());
    }
    let (_, fingerprint_openssh) = fingerprint_of(&key)?;

    Ok(RsaOutcome {
        output,
        scheme: scheme.label().into(),
        key_bits: k * 8,
        fingerprint_openssh,
        max_bytes: scheme.max_bytes(k).unwrap_or(0),
        payload_bytes: data.len(),
        elapsed_ms: started.elapsed().as_millis() as u64,
        notes,
    })
}

pub fn rsa_decrypt(spec: &RsaSpec, private_pem: &str, payload: &str) -> Result<RsaOutcome> {
    let started = Instant::now();
    let key = parse_private_key(&normalize_pem(private_pem))?;
    let k = key.size();
    let trimmed = payload.trim();
    if trimmed.is_empty() {
        return Err(IktError::Invalid("先粘贴要解密的内容".into()));
    }

    let mut notes = Vec::new();
    let (plain, scheme) = if trimmed.starts_with(HYBRID_PREFIX) {
        (hybrid_open(&key, trimmed)?, Scheme::Hybrid)
    } else {
        let scheme = Scheme::parse(&spec.scheme)?;
        if scheme == Scheme::Hybrid {
            return Err(IktError::Invalid(
                "混合方案只认得 ikt3. 开头的密文；裸密文请选择 OAEP 或 PKCS#1 v1.5".into(),
            ));
        }
        let cipher = decode_bytes(trimmed, &spec.input_encoding, k)?;
        if cipher.len() != k {
            return Err(IktError::Invalid(format!(
                "密文应为 {k} 字节（{} 位密钥），实际 {} 字节",
                k * 8,
                cipher.len()
            )));
        }
        let plain = match scheme {
            Scheme::OaepSha256 => key.decrypt(Oaep::new::<Sha256>(), &cipher),
            Scheme::OaepSha1 => key.decrypt(Oaep::new::<Sha1>(), &cipher),
            Scheme::Pkcs1v15 => key.decrypt(Pkcs1v15Encrypt, &cipher),
            Scheme::Hybrid => unreachable!(),
        }
        .map_err(|_| {
            IktError::Invalid("解密失败：私钥与密文不匹配，或方案选错了".to_string())
        })?;
        (plain, scheme)
    };

    let (output, note) = text_or_hex(&plain);
    if let Some(n) = note {
        notes.push(n);
    }
    if scheme == Scheme::Pkcs1v15 {
        notes.push("PKCS#1 v1.5 解密不校验填充结构，只用于兼容老系统".into());
    }
    if scheme == Scheme::Hybrid {
        notes.push("已校验 AES-GCM 认证标签，密文被改动会直接失败".into());
    }
    let public = RsaPublicKey::from(&key);
    let (_, fingerprint_openssh) = fingerprint_of(&public)?;

    Ok(RsaOutcome {
        output,
        scheme: scheme.label().into(),
        key_bits: k * 8,
        fingerprint_openssh,
        max_bytes: 0,
        payload_bytes: plain.len(),
        elapsed_ms: started.elapsed().as_millis() as u64,
        notes,
    })
}

// ---------------------------------------------------------------------------
// 混合封装
// ---------------------------------------------------------------------------

fn hybrid_seal(key: &RsaPublicKey, data: &[u8]) -> Result<String> {
    let mut rng = thread_rng();
    let mut aes_key = [0u8; AES_KEY_LEN];
    let mut iv = [0u8; GCM_IV_LEN];
    rng.fill_bytes(&mut aes_key);
    rng.fill_bytes(&mut iv);

    let cipher = Aes256Gcm::new_from_slice(&aes_key)
        .map_err(|_| IktError::Internal("会话密钥初始化失败".into()))?;
    let sealed = cipher
        .encrypt(Nonce::from_slice(&iv), data)
        .map_err(|_| IktError::Internal("AES-GCM 加密失败".into()))?;
    let wrapped = key
        .encrypt(&mut rng, Oaep::new::<Sha256>(), &aes_key)
        .map_err(|e| IktError::Invalid(format!("会话密钥包裹失败：{e}")))?;
    aes_key.zeroize();

    Ok(format!(
        "{HYBRID_PREFIX}{}.{}.{}",
        B64.encode(wrapped),
        B64.encode(iv),
        B64.encode(sealed)
    ))
}

fn hybrid_open(key: &RsaPrivateKey, payload: &str) -> Result<Vec<u8>> {
    let body = payload.trim_start_matches(HYBRID_PREFIX);
    let seg: Vec<&str> = body.split('.').collect();
    if seg.len() != 3 {
        return Err(IktError::Invalid(
            "ikt3 密文格式应为 ikt3.<包裹密钥>.<IV>.<正文>".into(),
        ));
    }
    let wrapped = decode_b64(seg[0])
        .ok_or_else(|| IktError::Invalid("ikt3 第一段不是合法 Base64".into()))?;
    let iv = decode_b64(seg[1]).ok_or_else(|| IktError::Invalid("ikt3 的 IV 不是合法 Base64".into()))?;
    let body_bytes =
        decode_b64(seg[2]).ok_or_else(|| IktError::Invalid("ikt3 正文不是合法 Base64".into()))?;
    if iv.len() != GCM_IV_LEN {
        return Err(IktError::Invalid(format!(
            "ikt3 的 IV 应为 {GCM_IV_LEN} 字节，实际 {} 字节",
            iv.len()
        )));
    }

    let mut aes_key = key
        .decrypt(Oaep::new::<Sha256>(), &wrapped)
        .map_err(|_| IktError::Invalid("会话密钥解不开：私钥与密文不匹配".to_string()))?;
    let cipher = Aes256Gcm::new_from_slice(&aes_key)
        .map_err(|_| IktError::Invalid("会话密钥长度异常".into()))?;
    let plain = cipher
        .decrypt(Nonce::from_slice(&iv), body_bytes.as_slice())
        .map_err(|_| IktError::Invalid("AES-GCM 认证失败：密文被改动过".to_string()))?;
    aes_key.zeroize();
    Ok(plain)
}

// ---------------------------------------------------------------------------
// PEM / 编码工具
// ---------------------------------------------------------------------------

/// 容忍从 JSON、.env 里粘过来的转义换行与首尾引号。
fn normalize_pem(raw: &str) -> String {
    let mut text = raw.trim().to_string();
    if text.len() >= 2 {
        let quoted = (text.starts_with('"') && text.ends_with('"'))
            || (text.starts_with('\'') && text.ends_with('\''));
        if quoted {
            text = text[1..text.len() - 1].to_string();
        }
    }
    if text.contains("\\n") && !text.contains('\n') {
        text = text.replace("\\n", "\n");
    }
    text.trim().to_string()
}

fn parse_public_key(pem: &str) -> Result<RsaPublicKey> {
    let text = pem.trim();
    if text.is_empty() {
        return Err(IktError::Invalid("先填入公钥".into()));
    }
    if text.contains("ENCRYPTED PRIVATE KEY") {
        return Err(IktError::Invalid(
            "不支持带口令的私钥，请先导出为明文 PEM".into(),
        ));
    }
    if text.contains("BEGIN RSA PUBLIC KEY") {
        return RsaPublicKey::from_pkcs1_pem(text)
            .map_err(|e| IktError::Invalid(format!("PKCS#1 公钥解析失败：{e}")));
    }
    if text.contains("BEGIN PUBLIC KEY") {
        return RsaPublicKey::from_public_key_pem(text)
            .map_err(|e| IktError::Invalid(format!("公钥解析失败：{e}")));
    }
    if text.contains("PRIVATE KEY") {
        return Ok(RsaPublicKey::from(&parse_private_key(text)?));
    }

    // 裸 Base64 DER：先按 SPKI 试，再按 PKCS#1 试
    let body: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    if let Some(der) = decode_b64(&body) {
        if let Ok(k) = RsaPublicKey::from_public_key_der(&der) {
            return Ok(k);
        }
        if let Ok(k) = RsaPublicKey::from_pkcs1_der(&der) {
            return Ok(k);
        }
    }
    Err(IktError::Invalid(
        "无法识别的公钥格式：支持 BEGIN PUBLIC KEY（SPKI）、BEGIN RSA PUBLIC KEY（PKCS#1）或 Base64 DER".into(),
    ))
}

fn parse_private_key(pem: &str) -> Result<RsaPrivateKey> {
    let text = pem.trim();
    if text.is_empty() {
        return Err(IktError::Invalid("先填入私钥".into()));
    }
    if text.contains("ENCRYPTED PRIVATE KEY") {
        return Err(IktError::Invalid(
            "不支持带口令的私钥，请先导出为明文 PEM".into(),
        ));
    }
    if text.contains("BEGIN RSA PRIVATE KEY") {
        return RsaPrivateKey::from_pkcs1_pem(text)
            .map_err(|e| IktError::Invalid(format!("PKCS#1 私钥解析失败：{e}")));
    }
    if text.contains("BEGIN PRIVATE KEY") {
        return RsaPrivateKey::from_pkcs8_pem(text)
            .map_err(|e| IktError::Invalid(format!("私钥解析失败：{e}")));
    }

    let body: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    if let Some(der) = decode_b64(&body) {
        if let Ok(k) = RsaPrivateKey::from_pkcs8_der(&der) {
            return Ok(k);
        }
        if let Ok(k) = RsaPrivateKey::from_pkcs1_der(&der) {
            return Ok(k);
        }
    }
    Err(IktError::Invalid(
        "无法识别的私钥格式：支持 BEGIN PRIVATE KEY（PKCS#8）、BEGIN RSA PRIVATE KEY（PKCS#1）或 Base64 DER"
            .into(),
    ))
}

fn fingerprint_of(public: &RsaPublicKey) -> Result<(String, String)> {
    let der = public
        .to_public_key_der()
        .map_err(|e| IktError::Internal(format!("公钥编码失败：{e}")))?;
    let digest = Sha256::digest(der.as_bytes());
    let raw: &[u8] = digest.as_ref();
    Ok((
        hex::encode(raw),
        format!("SHA256:{}", URL_SAFE_NO_PAD.encode(raw)),
    ))
}

fn encode_bytes(bytes: &[u8], encoding: &str) -> String {
    if encoding.trim().eq_ignore_ascii_case("hex") {
        hex::encode(bytes)
    } else {
        B64.encode(bytes)
    }
}

/// 解密输入：auto 模式下用「长度必须等于模长」来消除 Base64 / Hex 的歧义。
fn decode_bytes(value: &str, encoding: &str, expected_len: usize) -> Result<Vec<u8>> {
    let enc = encoding.trim().to_ascii_lowercase();
    let cleaned: String = value.chars().filter(|c| !c.is_whitespace()).collect();
    let hex_like = !cleaned.is_empty()
        && cleaned.len() % 2 == 0
        && cleaned.chars().all(|c| c.is_ascii_hexdigit());

    match enc.as_str() {
        "hex" => hex::decode(cleaned.trim_start_matches("0x"))
            .map_err(|_| IktError::Invalid("密文不是合法的十六进制字符串".into())),
        "base64" => decode_b64(&cleaned)
            .ok_or_else(|| IktError::Invalid("密文不是合法的 Base64".into())),
        _ => {
            let mut candidates: Vec<Vec<u8>> = Vec::new();
            if hex_like {
                if let Ok(v) = hex::decode(&cleaned) {
                    candidates.push(v);
                }
            }
            if let Some(v) = decode_b64(&cleaned) {
                candidates.push(v);
            }
            if let Some(hit) = candidates.iter().find(|v| v.len() == expected_len) {
                return Ok(hit.clone());
            }
            candidates.into_iter().next().ok_or_else(|| {
                IktError::Invalid("密文既不是合法 Base64 也不是 Hex".to_string())
            })
        }
    }
}

fn decode_b64(value: &str) -> Option<Vec<u8>> {
    use base64::engine::general_purpose::{STANDARD_NO_PAD, URL_SAFE};
    let cleaned: String = value.chars().filter(|c| !c.is_whitespace()).collect();
    if cleaned.is_empty() {
        return None;
    }
    let unpadded = cleaned.trim_end_matches('=');
    B64.decode(&cleaned)
        .or_else(|_| STANDARD_NO_PAD.decode(unpadded))
        .or_else(|_| URL_SAFE.decode(unpadded))
        .or_else(|_| URL_SAFE_NO_PAD.decode(unpadded))
        .ok()
}

fn text_or_hex(bytes: &[u8]) -> (String, Option<String>) {
    match std::str::from_utf8(bytes) {
        Ok(text) if !text.contains('\u{0}') => (text.to_string(), None),
        _ => (
            hex::encode(bytes),
            Some("解出的内容不是合法 UTF-8，已按 Hex 展示".to_string()),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 2048 位密钥生成在 debug 下不便宜，整个测试模块共用一对。
    fn shared() -> &'static RsaKeyPair {
        static PAIR: std::sync::OnceLock<RsaKeyPair> = std::sync::OnceLock::new();
        PAIR.get_or_init(|| generate_keypair(2048).unwrap())
    }

    #[test]
    fn pem_headers_are_canonical() {
        let pair = shared();
        assert!(pair.public_pem.starts_with("-----BEGIN PUBLIC KEY-----"));
        assert!(pair.private_pem.starts_with("-----BEGIN PRIVATE KEY-----"));
        assert!(pair.public_pkcs1_pem.starts_with("-----BEGIN RSA PUBLIC KEY-----"));
        assert!(pair.private_pkcs1_pem.starts_with("-----BEGIN RSA PRIVATE KEY-----"));
        assert_eq!(pair.bits, 2048);
        assert_eq!(pair.exponent, "65537");
        assert_eq!(pair.fingerprint_sha256.len(), 64);
        assert!(pair.fingerprint_openssh.starts_with("SHA256:"));
    }

    #[test]
    fn rejects_unsupported_bits() {
        assert!(generate_keypair(1024).is_err());
        assert!(generate_keypair(8192).is_err());
    }

    #[test]
    fn oaep_roundtrip_utf8() {
        let pair = shared();
        let spec = RsaSpec::default();
        let text = "中文 & emoji 🔐";
        let enc = rsa_encrypt(&spec, &pair.public_pem, text).unwrap();
        assert!(!enc.output.contains('\n'));
        let dec = rsa_decrypt(&spec, &pair.private_pem, &enc.output).unwrap();
        assert_eq!(dec.output, text);
        assert_eq!(dec.payload_bytes, text.len());
    }

    #[test]
    fn pkcs1v15_roundtrip_hex_output() {
        let pair = shared();
        let spec = RsaSpec {
            scheme: "pkcs1v15".into(),
            output_encoding: "hex".into(),
            ..Default::default()
        };
        let enc = rsa_encrypt(&spec, &pair.public_pem, "bench").unwrap();
        assert_eq!(enc.output.len(), 256 * 2);
        let dec = rsa_decrypt(&spec, &pair.private_pem, &enc.output).unwrap();
        assert_eq!(dec.output, "bench");
    }

    #[test]
    fn hybrid_handles_long_text() {
        let pair = shared();
        let spec = RsaSpec {
            scheme: "hybrid".into(),
            ..Default::default()
        };
        let payload = "长报文 ".repeat(3000);
        let enc = rsa_encrypt(&spec, &pair.public_pem, &payload).unwrap();
        assert!(enc.output.starts_with("ikt3."));
        assert_eq!(enc.max_bytes, 0);
        let dec = rsa_decrypt(&spec, &pair.private_pem, &enc.output).unwrap();
        assert_eq!(dec.output, payload);
    }

    #[test]
    fn oaep_rejects_oversized_payload() {
        let pair = shared();
        let too_long = "a".repeat(191); // 2048 位上限 190 字节
        let err = rsa_encrypt(&RsaSpec::default(), &pair.public_pem, &too_long).unwrap_err();
        assert!(err.to_string().contains("190"));
    }

    #[test]
    fn wrong_key_fails_cleanly() {
        let pair = shared();
        let other = generate_keypair(2048).unwrap();
        let enc = rsa_encrypt(&RsaSpec::default(), &pair.public_pem, "secret").unwrap();
        let err = rsa_decrypt(&RsaSpec::default(), &other.private_pem, &enc.output).unwrap_err();
        assert!(err.to_string().contains("解密失败"));
    }

    #[test]
    fn private_pem_can_encrypt_too() {
        let pair = shared();
        let enc = rsa_encrypt(&RsaSpec::default(), &pair.private_pem, "ok").unwrap();
        let dec = rsa_decrypt(&RsaSpec::default(), &pair.private_pem, &enc.output).unwrap();
        assert_eq!(dec.output, "ok");
    }

    #[test]
    fn accepts_escaped_newlines_and_quotes() {
        let pair = shared();
        let escaped = format!("\"{}\"", pair.public_pem.replace('\n', "\\n"));
        let enc = rsa_encrypt(&RsaSpec::default(), &escaped, "x").unwrap();
        assert!(!enc.output.is_empty());
    }

    #[test]
    fn auto_detect_picks_right_encoding() {
        let pair = shared();
        let enc = rsa_encrypt(
            &RsaSpec {
                scheme: "oaep-sha256".into(),
                output_encoding: "hex".into(),
                ..Default::default()
            },
            &pair.public_pem,
            "auto",
        )
        .unwrap();
        let dec = rsa_decrypt(&RsaSpec::default(), &pair.private_pem, &enc.output).unwrap();
        assert_eq!(dec.output, "auto");
    }

    #[test]
    fn inspect_reports_kind_and_bits() {
        let pair = shared();
        let pub_info = inspect(&pair.public_pem).unwrap();
        assert_eq!(pub_info.kind, "public");
        assert_eq!(pub_info.bits, 2048);
        let priv_info = inspect(&pair.private_pem).unwrap();
        assert_eq!(priv_info.kind, "private");
        assert_eq!(priv_info.fingerprint_sha256, pair.fingerprint_sha256);
        assert!(inspect("not a key").is_err());
    }

    #[test]
    fn serializes_camel_case_contract() {
        let pair = shared();
        let v = serde_json::to_value(pair).unwrap();
        for key in [
            "publicPem",
            "privatePem",
            "publicPkcs1Pem",
            "privatePkcs1Pem",
            "fingerprintSha256",
            "fingerprintOpenssh",
            "elapsedMs",
        ] {
            assert!(v.get(key).is_some(), "缺字段 {key}");
        }
    }
}
