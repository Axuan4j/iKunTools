use aes::cipher::{
    block_padding::{Pkcs7, ZeroPadding},
    consts::{U12, U16},
    Block, BlockCipher, BlockDecryptMut, BlockEncryptMut, KeyInit, KeyIvInit, StreamCipher,
};
use aes::{Aes128, Aes192, Aes256};
use aes_gcm::aead::Aead;
use aes_gcm::{AesGcm, Nonce};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use hmac::{Hmac, Mac};
use md5::Md5;
use sha1::Sha1;
use sha2::{Digest, Sha256, Sha512};
use zeroize::Zeroize;

use crate::{IktError, Result};

/// 分组算法标识。前端用同一份列表渲染下拉框，避免两边各维护一份。
pub const HASH_ALGORITHMS: [&str; 6] = ["md5", "sha1", "sha256", "sha512", "blake3", "hmac-sha256"];

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HashOutcome {
    pub algorithm: String,
    pub hex: String,
    pub base64: String,
    pub bits: usize,
    pub bytes: usize,
}

fn outcome(algorithm: &str, raw: &[u8]) -> HashOutcome {
    HashOutcome {
        algorithm: algorithm.to_string(),
        hex: hex::encode(raw),
        base64: B64.encode(raw),
        bits: raw.len() * 8,
        bytes: raw.len(),
    }
}

pub fn hash(algorithm: &str, input: &str) -> Result<HashOutcome> {
    let data = input.as_bytes();
    let alg = algorithm.to_ascii_lowercase();
    let raw: Vec<u8> = match alg.as_str() {
        "md5" => Md5::digest(data).to_vec(),
        "sha1" => Sha1::digest(data).to_vec(),
        "sha256" => Sha256::digest(data).to_vec(),
        "sha512" => Sha512::digest(data).to_vec(),
        "blake3" => blake3::hash(data).as_bytes().to_vec(),
        other => {
            return Err(IktError::Invalid(format!(
                "不支持的算法：{other}。可用：md5 / sha1 / sha256 / sha512 / blake3"
            )))
        }
    };
    Ok(outcome(&alg, &raw))
}

pub fn hmac_sha256(key: &str, input: &str) -> Result<HashOutcome> {
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(key.as_bytes())
        .map_err(|_| IktError::Invalid("HMAC 密钥不可用".into()))?;
    mac.update(input.as_bytes());
    Ok(outcome("hmac-sha256", &mac.finalize().into_bytes()))
}

/// 任意一种摘要算法，带上 HMAC 分支，供 UI 一个入口调用。
pub fn digest(algorithm: &str, input: &str, key: Option<&str>) -> Result<HashOutcome> {
    if algorithm.eq_ignore_ascii_case("hmac-sha256") {
        let k = key.unwrap_or("");
        if k.is_empty() {
            return Err(IktError::Invalid("HMAC-SHA256 需要提供密钥".into()));
        }
        return hmac_sha256(k, input);
    }
    hash(algorithm, input)
}

// ---------------------------------------------------------------------------
// AES
// ---------------------------------------------------------------------------

/// 前端下拉框直接复用这几份列表，避免两边各维护一份取值。
pub const AES_MODES: [&str; 4] = ["gcm", "cbc", "ctr", "ecb"];
pub const AES_PADDINGS: [&str; 3] = ["pkcs7", "zero", "none"];
pub const AES_ENCODINGS: [&str; 3] = ["utf8", "hex", "base64"];

const AES_PREFIX_V1: &str = "ikt1.";
const AES_PREFIX_V2: &str = "ikt2.";
const KDF_ITERATIONS: u32 = 120_000;
const SALT_LEN: usize = 16;
const GCM_NONCE_LEN: usize = 12;
/// 部分 Java / BouncyCastle 实现用 16 字节 GCM nonce，一并支持。
const GCM_NONCE_LEN_ALT: usize = 16;
const BLOCK_LEN: usize = 16;

type Aes128Gcm12 = AesGcm<Aes128, U12>;
type Aes128Gcm16 = AesGcm<Aes128, U16>;
type Aes192Gcm12 = AesGcm<Aes192, U12>;
type Aes192Gcm16 = AesGcm<Aes192, U16>;
type Aes256Gcm12 = AesGcm<Aes256, U12>;
type Aes256Gcm16 = AesGcm<Aes256, U16>;

/// 一次加/解密的全部参数。前端原样传过来，Rust 侧负责解析与校验。
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AesSpec {
    /// gcm | cbc | ctr | ecb
    pub mode: String,
    /// password（口令派生）| raw（原始密钥）
    pub key_mode: String,
    /// 口令，或原始密钥本身
    pub secret: String,
    /// 原始密钥的解释方式：utf8 | hex | base64
    pub key_encoding: String,
    /// random | custom
    pub iv_mode: String,
    /// 自定义 IV / nonce
    pub iv: String,
    /// IV 的解释方式：utf8 | hex | base64
    pub iv_encoding: String,
    /// pkcs7 | zero | none
    pub padding: String,
    /// 裸密文的输出编码：base64 | hex
    pub output_encoding: String,
}

impl Default for AesSpec {
    fn default() -> Self {
        Self {
            mode: "gcm".into(),
            key_mode: "password".into(),
            secret: String::new(),
            key_encoding: "utf8".into(),
            iv_mode: "random".into(),
            iv: String::new(),
            iv_encoding: "hex".into(),
            padding: "pkcs7".into(),
            output_encoding: "base64".into(),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AesOutcome {
    /// 加密：密文；解密：明文
    pub output: String,
    /// 实际生效的算法，例如 AES-256-CBC
    pub algorithm: String,
    pub key_bits: usize,
    /// 本次实际使用的 IV（十六进制），便于裸密文互通时抄走
    pub iv_hex: String,
    /// IV 是否已写进密文封装（口令模式为 true）
    pub iv_embedded: bool,
    pub padding: String,
    /// 参数被自动纠正时给出的提示
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Gcm,
    Cbc,
    Ctr,
    Ecb,
}

impl Mode {
    fn parse(raw: &str) -> Result<Self> {
        let v = raw.trim().to_ascii_lowercase();
        let v = v.strip_prefix("aes-").unwrap_or(&v);
        let v = v
            .strip_prefix("256-")
            .or_else(|| v.strip_prefix("192-"))
            .or_else(|| v.strip_prefix("128-"))
            .unwrap_or(v);
        match v {
            "gcm" | "" => Ok(Mode::Gcm),
            "cbc" => Ok(Mode::Cbc),
            "ctr" => Ok(Mode::Ctr),
            "ecb" => Ok(Mode::Ecb),
            other => Err(IktError::Invalid(format!(
                "不支持的模式：{other}。可用：gcm / cbc / ctr / ecb"
            ))),
        }
    }

    fn id(self) -> &'static str {
        match self {
            Mode::Gcm => "gcm",
            Mode::Cbc => "cbc",
            Mode::Ctr => "ctr",
            Mode::Ecb => "ecb",
        }
    }

    fn label(self, key_bits: usize) -> String {
        format!("AES-{key_bits}-{}", self.id().to_ascii_uppercase())
    }

    /// 该模式默认的 IV 长度
    fn iv_len(self) -> usize {
        match self {
            Mode::Gcm => GCM_NONCE_LEN,
            _ => BLOCK_LEN,
        }
    }

    fn check_iv(self, len: usize) -> Result<()> {
        let ok = match self {
            Mode::Gcm => len == GCM_NONCE_LEN || len == GCM_NONCE_LEN_ALT,
            _ => len == BLOCK_LEN,
        };
        if ok {
            return Ok(());
        }
        let want = match self {
            Mode::Gcm => "12（推荐）或 16",
            _ => "16",
        };
        Err(IktError::Invalid(format!(
            "{} 的 IV 需要 {want} 字节，当前 {len} 字节",
            self.id().to_ascii_uppercase()
        )))
    }

    fn is_stream(self) -> bool {
        matches!(self, Mode::Gcm | Mode::Ctr)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Padding {
    Pkcs7,
    Zero,
    None,
}

impl Padding {
    fn parse(raw: &str) -> Result<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "pkcs7" | "pkcs5" | "pkcs#7" | "" => Ok(Padding::Pkcs7),
            "zero" | "zeros" | "zeropadding" => Ok(Padding::Zero),
            "none" | "no" | "nopadding" => Ok(Padding::None),
            other => Err(IktError::Invalid(format!(
                "不支持的填充方式：{other}。可用：pkcs7 / zero / none"
            ))),
        }
    }

    fn id(self) -> &'static str {
        match self {
            Padding::Pkcs7 => "pkcs7",
            Padding::Zero => "zero",
            Padding::None => "none",
        }
    }
}

fn derive_key(password: &str, salt: &[u8]) -> [u8; 32] {
    let mut key = [0u8; 32];
    pbkdf2::pbkdf2_hmac::<Sha256>(password.as_bytes(), salt, KDF_ITERATIONS, &mut key);
    key
}

pub fn aes_encrypt(spec: &AesSpec, plaintext: &str) -> Result<AesOutcome> {
    let mode = Mode::parse(&spec.mode)?;
    let mut notes: Vec<String> = Vec::new();

    // ---- 密钥 ----
    let (mut key, salt) = if spec.key_mode.eq_ignore_ascii_case("raw") {
        let raw = decode_value(&spec.secret, &spec.key_encoding, "密钥")?;
        if !matches!(raw.len(), 16 | 24 | 32) {
            return Err(bad_key_len(raw.len()));
        }
        (raw, None)
    } else {
        if spec.secret.is_empty() {
            return Err(IktError::Invalid("请先设置一个口令".into()));
        }
        let salt: [u8; SALT_LEN] = rand::random();
        (derive_key(&spec.secret, &salt).to_vec(), Some(salt))
    };

    // ---- 填充 ----
    let padding = if mode.is_stream() {
        if !matches!(spec.padding.as_str(), "" | "none") {
            notes.push("GCM / CTR 是流式模式，不需要填充，已忽略填充设置".into());
        }
        Padding::None
    } else {
        Padding::parse(&spec.padding)?
    };

    // ---- IV ----
    let iv = if mode == Mode::Ecb {
        if spec.iv_mode.eq_ignore_ascii_case("custom") && !spec.iv.trim().is_empty() {
            notes.push("ECB 不使用 IV，已忽略填写的值".into());
        }
        None
    } else {
        match spec.iv_mode.as_str() {
            "custom" => {
                if spec.iv.trim().is_empty() {
                    return Err(IktError::Invalid("已选择自定义 IV，但内容为空".into()));
                }
                let v = decode_value(&spec.iv, &spec.iv_encoding, "IV")?;
                mode.check_iv(v.len())?;
                Some(v)
            }
            _ => Some(random_bytes(mode.iv_len())),
        }
    };

    let ciphertext = encrypt_bytes(mode, &key, iv.as_deref(), padding, plaintext.as_bytes())?;
    let key_bits = key.len() * 8;
    key.zeroize();

    let (output, iv_embedded) = match salt {
        Some(salt) => {
            if spec.output_encoding.eq_ignore_ascii_case("hex") {
                notes.push("口令模式输出自描述封装，段内固定 Base64".into());
            }
            (
                format!(
                    "{AES_PREFIX_V2}{}.{}.{}.{}.{}",
                    mode.id(),
                    padding.id(),
                    B64.encode(salt),
                    iv.as_ref().map(|v| B64.encode(v)).unwrap_or_default(),
                    B64.encode(&ciphertext)
                ),
                iv.is_some(),
            )
        }
        None => (encode_value(&ciphertext, &spec.output_encoding), false),
    };

    Ok(AesOutcome {
        output,
        algorithm: mode.label(key_bits),
        key_bits,
        iv_hex: iv.as_ref().map(hex::encode).unwrap_or_default(),
        iv_embedded,
        padding: padding.id().to_string(),
        notes,
    })
}

pub fn aes_decrypt(spec: &AesSpec, payload: &str) -> Result<AesOutcome> {
    let payload = payload.trim();
    if payload.is_empty() {
        return Err(IktError::Invalid("请先粘贴要解密的密文".into()));
    }

    // 1) ikt2 自描述封装：算法 / 填充 / salt / IV 都在密文里，界面参数只用来提供口令
    if let Some(rest) = payload.strip_prefix(AES_PREFIX_V2) {
        let segs: Vec<&str> = rest.split('.').collect();
        if segs.len() != 5 {
            return Err(IktError::Invalid(
                "ikt2 密文应为 ikt2.算法.填充.salt.iv.密文 共 6 段".into(),
            ));
        }
        let mode = Mode::parse(segs[0])?;
        let padding = if mode.is_stream() {
            Padding::None
        } else {
            Padding::parse(segs[1])?
        };
        if !spec.key_mode.eq_ignore_ascii_case("password") || spec.secret.is_empty() {
            return Err(IktError::Invalid(
                "这是口令模式产生的 ikt2 封装，请把密钥方式切到「口令」并填写口令".into(),
            ));
        }
        let salt = b64_part(segs[2], "salt")?;
        if salt.len() != SALT_LEN {
            return Err(IktError::Invalid("salt 长度异常".into()));
        }
        let iv = if segs[3].is_empty() {
            None
        } else {
            let v = b64_part(segs[3], "iv")?;
            mode.check_iv(v.len())?;
            Some(v)
        };
        if mode != Mode::Ecb && iv.is_none() {
            return Err(IktError::Invalid("该模式缺少 IV 段".into()));
        }
        let ct = b64_part(segs[4], "ciphertext")?;

        let mut key = derive_key(&spec.secret, &salt).to_vec();
        let key_bits = key.len() * 8;
        let plain = decrypt_bytes(mode, &key, iv.as_deref(), padding, &ct)?;
        key.zeroize();
        return Ok(plain_outcome(
            plain,
            mode,
            key_bits,
            iv.as_deref(),
            padding,
            true,
            Vec::new(),
        ));
    }

    // 2) 旧版 ikt1：等价于 GCM + 无填充 + 口令派生 256 位密钥
    if let Some(rest) = payload.strip_prefix(AES_PREFIX_V1) {
        let segs: Vec<&str> = rest.split('.').collect();
        if segs.len() != 3 {
            return Err(IktError::Invalid("ikt1 密文应为 ikt1.salt.nonce.密文".into()));
        }
        if spec.secret.is_empty() {
            return Err(IktError::Invalid("请填写口令".into()));
        }
        let salt = b64_part(segs[0], "salt")?;
        let nonce = b64_part(segs[1], "nonce")?;
        let ct = b64_part(segs[2], "ciphertext")?;
        if salt.len() != SALT_LEN || nonce.len() != GCM_NONCE_LEN {
            return Err(IktError::Invalid("salt / nonce 长度异常".into()));
        }
        let mut key = derive_key(&spec.secret, &salt).to_vec();
        let plain = decrypt_bytes(Mode::Gcm, &key, Some(&nonce), Padding::None, &ct)?;
        key.zeroize();
        return Ok(plain_outcome(
            plain,
            Mode::Gcm,
            256,
            Some(&nonce),
            Padding::None,
            true,
            vec!["按旧版 ikt1 封装解密".into()],
        ));
    }

    // 3) 裸密文：完全按界面参数来（原始密钥 + 自定 IV），用于和别的系统互通
    let mode = Mode::parse(&spec.mode)?;
    let mut notes: Vec<String> = Vec::new();
    let padding = if mode.is_stream() {
        Padding::None
    } else {
        Padding::parse(&spec.padding)?
    };

    let mut key = if spec.key_mode.eq_ignore_ascii_case("raw") {
        let raw = decode_value(&spec.secret, &spec.key_encoding, "密钥")?;
        if !matches!(raw.len(), 16 | 24 | 32) {
            return Err(bad_key_len(raw.len()));
        }
        raw
    } else {
        return Err(IktError::Invalid(
            "裸密文没有 salt，无法用口令派生密钥；请使用原始密钥，或粘贴 ikt2 封装密文".into(),
        ));
    };
    let key_bits = key.len() * 8;

    let iv = if mode == Mode::Ecb {
        if !spec.iv.trim().is_empty() && spec.iv_mode == "custom" {
            notes.push("ECB 不使用 IV，已忽略".into());
        }
        None
    } else {
        if spec.iv.trim().is_empty() {
            return Err(IktError::Invalid(
                "裸密文解密需要提供 IV：切到「自定」并填入加密时使用的值".into(),
            ));
        }
        let v = decode_value(&spec.iv, &spec.iv_encoding, "IV")?;
        mode.check_iv(v.len())?;
        Some(v)
    };

    let ct_candidates = ciphertext_candidates(payload, &spec.output_encoding);
    if ct_candidates.is_empty() {
        return Err(IktError::Invalid("密文既不是合法 Base64 也不是 Hex".into()));
    }
    let mut last_err: Option<IktError> = None;
    for ct in ct_candidates {
        match decrypt_bytes(mode, &key, iv.as_deref(), padding, &ct) {
            Ok(plain) => {
                key.zeroize();
                return Ok(plain_outcome(
                    plain,
                    mode,
                    key_bits,
                    iv.as_deref(),
                    padding,
                    false,
                    notes,
                ));
            }
            Err(e) => last_err = Some(e),
        }
    }
    key.zeroize();
    Err(last_err.unwrap_or_else(|| IktError::Invalid("解密失败".into())))
}

fn plain_outcome(
    plain: Vec<u8>,
    mode: Mode,
    key_bits: usize,
    iv: Option<&[u8]>,
    padding: Padding,
    iv_embedded: bool,
    mut notes: Vec<String>,
) -> AesOutcome {
    let (output, warn) = match String::from_utf8(plain) {
        Ok(s) => (s, None),
        Err(e) => (
            hex::encode(e.as_bytes()),
            Some("明文不是合法 UTF-8，已按十六进制展示".to_string()),
        ),
    };
    if let Some(w) = warn {
        notes.push(w);
    }
    AesOutcome {
        output,
        algorithm: mode.label(key_bits),
        key_bits,
        iv_hex: iv.map(hex::encode).unwrap_or_default(),
        iv_embedded,
        padding: padding.id().to_string(),
        notes,
    }
}

// ---------------------------------------------------------------------------
// 底层分组运算
// ---------------------------------------------------------------------------

fn encrypt_bytes(
    mode: Mode,
    key: &[u8],
    iv: Option<&[u8]>,
    padding: Padding,
    data: &[u8],
) -> Result<Vec<u8>> {
    match mode {
        Mode::Gcm => gcm_encrypt_dispatch(key, require_iv(iv)?, data, true),
        Mode::Cbc => cbc_encrypt(key, require_iv(iv)?, data, padding),
        Mode::Ctr => ctr_xor(key, require_iv(iv)?, data),
        Mode::Ecb => ecb_encrypt(key, data, padding),
    }
}

fn decrypt_bytes(
    mode: Mode,
    key: &[u8],
    iv: Option<&[u8]>,
    padding: Padding,
    data: &[u8],
) -> Result<Vec<u8>> {
    match mode {
        Mode::Gcm => gcm_encrypt_dispatch(key, require_iv(iv)?, data, false),
        Mode::Cbc => cbc_decrypt(key, require_iv(iv)?, data, padding),
        Mode::Ctr => ctr_xor(key, require_iv(iv)?, data),
        Mode::Ecb => ecb_decrypt(key, data, padding),
    }
}

fn require_iv(iv: Option<&[u8]>) -> Result<&[u8]> {
    iv.ok_or_else(|| IktError::Invalid("该模式需要 IV".into()))
}

fn gcm_seal<A>(key: &[u8], nonce: &[u8], data: &[u8], encrypting: bool) -> Result<Vec<u8>>
where
    A: KeyInit + Aead,
{
    let cipher = A::new_from_slice(key).map_err(|_| bad_key_len(key.len()))?;
    let nonce_ref = Nonce::<A::NonceSize>::from_slice(nonce);
    if encrypting {
        cipher
            .encrypt(nonce_ref, data)
            .map_err(|_| IktError::Internal("GCM 加密失败".into()))
    } else {
        cipher.decrypt(nonce_ref, data).map_err(|_| {
            IktError::Invalid("解密失败：密钥或 IV 不正确，或密文已损坏（GCM 会校验完整性）".into())
        })
    }
}

fn gcm_encrypt_dispatch(key: &[u8], nonce: &[u8], data: &[u8], encrypting: bool) -> Result<Vec<u8>> {
    match (key.len(), nonce.len()) {
        (16, 12) => gcm_seal::<Aes128Gcm12>(key, nonce, data, encrypting),
        (16, 16) => gcm_seal::<Aes128Gcm16>(key, nonce, data, encrypting),
        (24, 12) => gcm_seal::<Aes192Gcm12>(key, nonce, data, encrypting),
        (24, 16) => gcm_seal::<Aes192Gcm16>(key, nonce, data, encrypting),
        (32, 12) => gcm_seal::<Aes256Gcm12>(key, nonce, data, encrypting),
        (32, 16) => gcm_seal::<Aes256Gcm16>(key, nonce, data, encrypting),
        (n, _) if !matches!(n, 16 | 24 | 32) => Err(bad_key_len(n)),
        (_, len) => Err(IktError::Invalid(format!(
            "GCM 的 IV 需要 12 或 16 字节，当前 {len} 字节"
        ))),
    }
}

fn cbc_encrypt(key: &[u8], iv: &[u8], data: &[u8], pad: Padding) -> Result<Vec<u8>> {
    match key.len() {
        16 => cbc_encrypt_for::<Aes128>(key, iv, data, pad),
        24 => cbc_encrypt_for::<Aes192>(key, iv, data, pad),
        32 => cbc_encrypt_for::<Aes256>(key, iv, data, pad),
        n => Err(bad_key_len(n)),
    }
}

fn cbc_decrypt(key: &[u8], iv: &[u8], data: &[u8], pad: Padding) -> Result<Vec<u8>> {
    match key.len() {
        16 => cbc_decrypt_for::<Aes128>(key, iv, data, pad),
        24 => cbc_decrypt_for::<Aes192>(key, iv, data, pad),
        32 => cbc_decrypt_for::<Aes256>(key, iv, data, pad),
        n => Err(bad_key_len(n)),
    }
}

fn ecb_encrypt(key: &[u8], data: &[u8], pad: Padding) -> Result<Vec<u8>> {
    match key.len() {
        16 => ecb_encrypt_for::<Aes128>(key, data, pad),
        24 => ecb_encrypt_for::<Aes192>(key, data, pad),
        32 => ecb_encrypt_for::<Aes256>(key, data, pad),
        n => Err(bad_key_len(n)),
    }
}

fn ecb_decrypt(key: &[u8], data: &[u8], pad: Padding) -> Result<Vec<u8>> {
    match key.len() {
        16 => ecb_decrypt_for::<Aes128>(key, data, pad),
        24 => ecb_decrypt_for::<Aes192>(key, data, pad),
        32 => ecb_decrypt_for::<Aes256>(key, data, pad),
        n => Err(bad_key_len(n)),
    }
}

fn ctr_xor(key: &[u8], iv: &[u8], data: &[u8]) -> Result<Vec<u8>> {
    match key.len() {
        16 => ctr_xor_for::<Aes128>(key, iv, data),
        24 => ctr_xor_for::<Aes192>(key, iv, data),
        32 => ctr_xor_for::<Aes256>(key, iv, data),
        n => Err(bad_key_len(n)),
    }
}

fn cbc_encrypt_for<C>(key: &[u8], iv: &[u8], data: &[u8], pad: Padding) -> Result<Vec<u8>>
where
    C: BlockCipher<BlockSize = U16> + BlockEncryptMut + KeyInit,
{
    let mut cipher = <cbc::Encryptor<C> as KeyIvInit>::new_from_slices(key, iv)
        .map_err(|_| IktError::Invalid("CBC 密钥或 IV 长度不正确".into()))?;
    Ok(match pad {
        Padding::Pkcs7 => cipher.encrypt_padded_vec_mut::<Pkcs7>(data),
        Padding::Zero => cipher.encrypt_padded_vec_mut::<ZeroPadding>(data),
        Padding::None => {
            ensure_aligned(data.len())?;
            let mut buf = data.to_vec();
            for chunk in buf.chunks_mut(BLOCK_LEN) {
                cipher.encrypt_block_mut(Block::<C>::from_mut_slice(chunk));
            }
            buf
        }
    })
}

fn cbc_decrypt_for<C>(key: &[u8], iv: &[u8], data: &[u8], pad: Padding) -> Result<Vec<u8>>
where
    C: BlockCipher<BlockSize = U16> + BlockDecryptMut + KeyInit,
{
    let mut cipher = <cbc::Decryptor<C> as KeyIvInit>::new_from_slices(key, iv)
        .map_err(|_| IktError::Invalid("CBC 密钥或 IV 长度不正确".into()))?;
    match pad {
        Padding::Pkcs7 => cipher.decrypt_padded_vec_mut::<Pkcs7>(data).map_err(|_| unpad_err()),
        Padding::Zero => cipher
            .decrypt_padded_vec_mut::<ZeroPadding>(data)
            .map_err(|_| unpad_err()),
        Padding::None => {
            ensure_aligned(data.len())?;
            let mut buf = data.to_vec();
            for chunk in buf.chunks_mut(BLOCK_LEN) {
                cipher.decrypt_block_mut(Block::<C>::from_mut_slice(chunk));
            }
            Ok(buf)
        }
    }
}

fn ecb_encrypt_for<C>(key: &[u8], data: &[u8], pad: Padding) -> Result<Vec<u8>>
where
    C: BlockCipher<BlockSize = U16> + BlockEncryptMut + KeyInit,
{
    let mut cipher = <ecb::Encryptor<C> as KeyInit>::new_from_slice(key)
        .map_err(|_| bad_key_len(key.len()))?;
    Ok(match pad {
        Padding::Pkcs7 => cipher.encrypt_padded_vec_mut::<Pkcs7>(data),
        Padding::Zero => cipher.encrypt_padded_vec_mut::<ZeroPadding>(data),
        Padding::None => {
            ensure_aligned(data.len())?;
            let mut buf = data.to_vec();
            for chunk in buf.chunks_mut(BLOCK_LEN) {
                cipher.encrypt_block_mut(Block::<C>::from_mut_slice(chunk));
            }
            buf
        }
    })
}

fn ecb_decrypt_for<C>(key: &[u8], data: &[u8], pad: Padding) -> Result<Vec<u8>>
where
    C: BlockCipher<BlockSize = U16> + BlockDecryptMut + KeyInit,
{
    let mut cipher = <ecb::Decryptor<C> as KeyInit>::new_from_slice(key)
        .map_err(|_| bad_key_len(key.len()))?;
    match pad {
        Padding::Pkcs7 => cipher.decrypt_padded_vec_mut::<Pkcs7>(data).map_err(|_| unpad_err()),
        Padding::Zero => cipher
            .decrypt_padded_vec_mut::<ZeroPadding>(data)
            .map_err(|_| unpad_err()),
        Padding::None => {
            ensure_aligned(data.len())?;
            let mut buf = data.to_vec();
            for chunk in buf.chunks_mut(BLOCK_LEN) {
                cipher.decrypt_block_mut(Block::<C>::from_mut_slice(chunk));
            }
            Ok(buf)
        }
    }
}

fn ctr_xor_for<C>(key: &[u8], iv: &[u8], data: &[u8]) -> Result<Vec<u8>>
where
    C: BlockCipher<BlockSize = U16> + aes::cipher::BlockEncrypt + KeyInit,
    ctr::Ctr128BE<C>: KeyIvInit + StreamCipher,
{
    let mut cipher = <ctr::Ctr128BE<C> as KeyIvInit>::new_from_slices(key, iv)
        .map_err(|_| IktError::Invalid("CTR 密钥或 IV 长度不正确".into()))?;
    let mut buf = data.to_vec();
    cipher.apply_keystream(&mut buf);
    Ok(buf)
}

fn ensure_aligned(len: usize) -> Result<()> {
    if len % BLOCK_LEN == 0 {
        return Ok(());
    }
    Err(IktError::Invalid(format!(
        "无填充要求输入长度是 {BLOCK_LEN} 的整数倍，当前 {len} 字节（差 {} 字节）",
        BLOCK_LEN - len % BLOCK_LEN
    )))
}

fn unpad_err() -> IktError {
    IktError::Invalid("解密失败：密钥或 IV 不正确，或填充校验未通过".into())
}

fn bad_key_len(n: usize) -> IktError {
    IktError::Invalid(format!(
        "AES 密钥需要 16 / 24 / 32 字节（对应 AES-128/192/256），当前 {n} 字节"
    ))
}

fn random_bytes(len: usize) -> Vec<u8> {
    use rand::RngCore;
    let mut buf = vec![0u8; len];
    rand::thread_rng().fill_bytes(&mut buf);
    buf
}

fn decode_value(value: &str, encoding: &str, label: &str) -> Result<Vec<u8>> {
    let v = value.trim();
    match encoding.trim().to_ascii_lowercase().as_str() {
        "hex" => {
            let cleaned: String = v
                .chars()
                .filter(|c| !c.is_whitespace() && *c != ':' && *c != '-')
                .collect();
            hex::decode(&cleaned)
                .map_err(|_| IktError::Invalid(format!("{label} 不是合法的十六进制字符串")))
        }
        "base64" => decode_b64_flexible(v)
            .map_err(|_| IktError::Invalid(format!("{label} 不是合法的 Base64"))),
        _ => Ok(v.as_bytes().to_vec()),
    }
}

fn encode_value(bytes: &[u8], encoding: &str) -> String {
    if encoding.trim().eq_ignore_ascii_case("hex") {
        hex::encode(bytes)
    } else {
        B64.encode(bytes)
    }
}

/// 密文编码的候选顺序：先按用户选的编码，再兜底另一种。
/// 这样「编码选错了但内容其实是另一种」也能解出来，省一次来回切换。
fn ciphertext_candidates(value: &str, hint: &str) -> Vec<Vec<u8>> {
    let cleaned: String = value.chars().filter(|c| !c.is_whitespace()).collect();
    let normalized: String = cleaned.chars().filter(|c| *c != ':' && *c != '-').collect();
    let as_b64 = decode_b64_flexible(&cleaned).ok();
    let as_hex = hex::decode(&normalized).ok();

    let ordered = if hint.trim().eq_ignore_ascii_case("hex") {
        [as_hex, as_b64]
    } else {
        [as_b64, as_hex]
    };

    let mut out: Vec<Vec<u8>> = Vec::new();
    for candidate in ordered.into_iter().flatten() {
        if !candidate.is_empty() && !out.contains(&candidate) {
            out.push(candidate);
        }
    }
    out
}

fn decode_b64_flexible(input: &str) -> std::result::Result<Vec<u8>, base64::DecodeError> {
    let norm = input.replace('-', "+").replace('_', "/");
    let trimmed = norm.trim_end_matches('=');
    let mut padded = trimmed.to_string();
    while padded.len() % 4 != 0 {
        padded.push('=');
    }
    B64.decode(padded.as_bytes())
}

fn b64_part(part: &str, label: &str) -> Result<Vec<u8>> {
    if part.is_empty() {
        return Ok(Vec::new());
    }
    B64.decode(part)
        .map_err(|_| IktError::Invalid(format!("{label} 段不是合法 Base64")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex_bytes(s: &str) -> Vec<u8> {
        hex::decode(s).unwrap()
    }

    #[test]
    fn sha256_matches_known_vector() {
        let out = hash("sha256", "abc").unwrap();
        assert_eq!(
            out.hex,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(out.bits, 256);
    }

    #[test]
    fn md5_matches_known_vector() {
        assert_eq!(hash("md5", "abc").unwrap().hex, "900150983cd24fb0d6963f7d28e17f72");
        assert_eq!(hash("sha1", "abc").unwrap().hex, "a9993e364706816aba3e25717850c26c9cd0d89d");
    }

    #[test]
    fn hmac_sha256_matches_rfc4231() {
        // RFC 4231 test case 2
        let out = hmac_sha256("Jefe", "what do ya want for nothing?").unwrap();
        assert_eq!(
            out.hex,
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
    }

    // ---- 分组模式：与 NIST SP 800-38A 向量对齐 ----

    #[test]
    fn cbc_128_matches_nist_vector() {
        let key = hex_bytes("2b7e151628aed2a6abf7158809cf4f3c");
        let iv = hex_bytes("000102030405060708090a0b0c0d0e0f");
        let pt = hex_bytes("6bc1bee22e409f96e93d7e117393172a");
        let ct = encrypt_bytes(Mode::Cbc, &key, Some(&iv), Padding::None, &pt).unwrap();
        assert_eq!(hex::encode(&ct), "7649abac8119b246cee98e9b12e9197d");
        assert_eq!(
            decrypt_bytes(Mode::Cbc, &key, Some(&iv), Padding::None, &ct).unwrap(),
            pt
        );
    }

    #[test]
    fn cbc_256_matches_nist_vector() {
        let key = hex_bytes("603deb1015ca71be2b73aef0857d77811f352c073b6108d72d9810a30914dff4");
        let iv = hex_bytes("000102030405060708090a0b0c0d0e0f");
        let pt = hex_bytes("6bc1bee22e409f96e93d7e117393172a");
        let ct = encrypt_bytes(Mode::Cbc, &key, Some(&iv), Padding::None, &pt).unwrap();
        assert_eq!(hex::encode(&ct), "f58c4c04d6e5f1ba779eabfb5f7bfbd6");
    }

    #[test]
    fn ctr_128_matches_nist_vector() {
        let key = hex_bytes("2b7e151628aed2a6abf7158809cf4f3c");
        let iv = hex_bytes("f0f1f2f3f4f5f6f7f8f9fafbfcfdfeff");
        let pt = hex_bytes("6bc1bee22e409f96e93d7e117393172a");
        let ct = encrypt_bytes(Mode::Ctr, &key, Some(&iv), Padding::None, &pt).unwrap();
        assert_eq!(hex::encode(&ct), "874d6191b620e3261bef6864990db6ce");
        assert_eq!(
            decrypt_bytes(Mode::Ctr, &key, Some(&iv), Padding::None, &ct).unwrap(),
            pt
        );
    }

    #[test]
    fn ecb_128_matches_nist_vector() {
        let key = hex_bytes("2b7e151628aed2a6abf7158809cf4f3c");
        let pt = hex_bytes("6bc1bee22e409f96e93d7e117393172aae2d8a571e03ac9c9eb76fac45af8e51");
        let ct = encrypt_bytes(Mode::Ecb, &key, None, Padding::None, &pt).unwrap();
        assert_eq!(
            hex::encode(&ct),
            "3ad77bb40d7a3660a89ecaf32466ef97f5d3d58503b9699de785895a96fdbaaf"
        );
        assert_eq!(
            decrypt_bytes(Mode::Ecb, &key, None, Padding::None, &ct).unwrap(),
            pt
        );
    }

    // ---- 参数组合 ----

    #[test]
    fn ikt2_cbc_pkcs7_roundtrip() {
        let spec = AesSpec {
            mode: "cbc".into(),
            key_mode: "password".into(),
            secret: "k1kun-2026".into(),
            ..Default::default()
        };
        let secret = "密码：中文也能加密 🔐，还有换行\n第二行";
        let enc = aes_encrypt(&spec, secret).unwrap();
        assert!(enc.output.starts_with("ikt2.cbc.pkcs7."));
        assert!(enc.iv_embedded);

        // 界面参数故意填错，仍应按密文里的描述解密
        let wrong_ui = AesSpec {
            mode: "gcm".into(),
            padding: "none".into(),
            ..spec.clone()
        };
        assert_eq!(aes_decrypt(&wrong_ui, &enc.output).unwrap().output, secret);
    }

    #[test]
    fn ikt2_ctr_roundtrip() {
        let spec = AesSpec {
            mode: "ctr".into(),
            key_mode: "password".into(),
            secret: "p".into(),
            ..Default::default()
        };
        let enc = aes_encrypt(&spec, "stream mode").unwrap();
        assert!(enc.output.starts_with("ikt2.ctr.none."));
        assert_eq!(aes_decrypt(&spec, &enc.output).unwrap().output, "stream mode");
    }

    #[test]
    fn raw_key_cbc_custom_iv_roundtrip() {
        let mut spec = AesSpec {
            mode: "cbc".into(),
            key_mode: "raw".into(),
            secret: "0123456789abcdef".into(),
            key_encoding: "utf8".into(),
            iv_mode: "custom".into(),
            iv: "abcdef0123456789".into(),
            iv_encoding: "utf8".into(),
            ..Default::default()
        };
        let enc = aes_encrypt(&spec, "hello 中文").unwrap();
        assert_eq!(enc.key_bits, 128);
        assert_eq!(enc.iv_hex, hex::encode(b"abcdef0123456789"));
        assert!(!enc.iv_embedded);

        spec.output_encoding = "base64".into();
        let enc = aes_encrypt(&spec, "hello 中文").unwrap();
        assert_eq!(aes_decrypt(&spec, &enc.output).unwrap().output, "hello 中文");
    }

    #[test]
    fn hex_output_can_be_decrypted() {
        let mut spec = AesSpec {
            mode: "cbc".into(),
            key_mode: "raw".into(),
            secret: "2b7e151628aed2a6abf7158809cf4f3c".into(),
            key_encoding: "hex".into(),
            iv_mode: "custom".into(),
            iv: "000102030405060708090a0b0c0d0e0f".into(),
            iv_encoding: "hex".into(),
            output_encoding: "hex".into(),
            ..Default::default()
        };
        let enc = aes_encrypt(&spec, "hex 输出").unwrap();
        assert!(enc.output.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(aes_decrypt(&spec, &enc.output).unwrap().output, "hex 输出");
        // 编码切到 base64 也应该能靠兜底解析成功
        spec.output_encoding = "base64".into();
        assert_eq!(aes_decrypt(&spec, &enc.output).unwrap().output, "hex 输出");
    }

    #[test]
    fn zero_padding_roundtrip() {
        let spec = AesSpec {
            mode: "ecb".into(),
            key_mode: "raw".into(),
            secret: "0123456789abcdef0123456789abcdef".into(),
            padding: "zero".into(),
            ..Default::default()
        };
        let enc = aes_encrypt(&spec, "zero padding").unwrap();
        assert_eq!(aes_decrypt(&spec, &enc.output).unwrap().output, "zero padding");
    }

    #[test]
    fn no_padding_rejects_unaligned() {
        let spec = AesSpec {
            mode: "ecb".into(),
            key_mode: "raw".into(),
            secret: "0123456789abcdef0123456789abcdef".into(),
            padding: "none".into(),
            ..Default::default()
        };
        let err = aes_encrypt(&spec, "not block aligned").unwrap_err().to_string();
        assert!(err.contains("整数倍"), "{err}");
    }

    #[test]
    fn gcm_accepts_16_byte_nonce() {
        let spec = AesSpec {
            mode: "gcm".into(),
            key_mode: "raw".into(),
            secret: "0123456789abcdef".into(),
            iv_mode: "custom".into(),
            iv: "00112233445566778899aabbccddeeff".into(),
            iv_encoding: "hex".into(),
            ..Default::default()
        };
        let enc = aes_encrypt(&spec, "16 字节 nonce").unwrap();
        assert_eq!(enc.iv_hex, "00112233445566778899aabbccddeeff");
        assert_eq!(aes_decrypt(&spec, &enc.output).unwrap().output, "16 字节 nonce");
    }

    #[test]
    fn wrong_password_fails() {
        let spec = AesSpec {
            mode: "cbc".into(),
            secret: "right".into(),
            ..Default::default()
        };
        let enc = aes_encrypt(&spec, "payload").unwrap();
        let bad = AesSpec {
            secret: "wrong".into(),
            ..spec
        };
        assert!(aes_decrypt(&bad, &enc.output).is_err());
    }

    #[test]
    fn legacy_ikt1_still_decrypts() {
        let salt: [u8; SALT_LEN] = rand::random();
        let nonce: [u8; GCM_NONCE_LEN] = rand::random();
        let key = derive_key("old-password", &salt);
        let ct = gcm_seal::<Aes256Gcm12>(&key, &nonce, "历史密文".as_bytes(), true).unwrap();
        let payload = format!(
            "{AES_PREFIX_V1}{}.{}.{}",
            B64.encode(salt),
            B64.encode(nonce),
            B64.encode(ct)
        );
        let spec = AesSpec {
            secret: "old-password".into(),
            ..Default::default()
        };
        assert_eq!(aes_decrypt(&spec, &payload).unwrap().output, "历史密文");
    }

    #[test]
    fn ikt2_without_password_is_rejected_clearly() {
        let spec = AesSpec {
            secret: "p".into(),
            ..Default::default()
        };
        let enc = aes_encrypt(&spec, "x").unwrap();
        let raw = AesSpec {
            key_mode: "raw".into(),
            secret: "0123456789abcdef".into(),
            ..Default::default()
        };
        let err = aes_decrypt(&raw, &enc.output).unwrap_err().to_string();
        assert!(err.contains("口令"), "{err}");
    }

    #[test]
    fn nonce_is_random() {
        let spec = AesSpec {
            secret: "p".into(),
            ..Default::default()
        };
        let a = aes_encrypt(&spec, "same").unwrap();
        let b = aes_encrypt(&spec, "same").unwrap();
        assert_ne!(a.output, b.output, "同口令同明文必须产出不同密文");
    }

    #[test]
    fn rejects_bad_key_length() {
        let spec = AesSpec {
            key_mode: "raw".into(),
            secret: "short".into(),
            ..Default::default()
        };
        let err = aes_encrypt(&spec, "x").unwrap_err().to_string();
        assert!(err.contains("16 / 24 / 32"), "{err}");
    }
}
