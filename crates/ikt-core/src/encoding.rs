use base64::{
    engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD},
    Engine,
};
use serde::{Deserialize, Serialize};

use crate::{IktError, Result};

/// Base64 编码。`url_safe` 用 `-` `_` 替代 `+` `/`，`no_pad` 去掉尾部 `=`。
pub fn base64_encode(input: &str, url_safe: bool, no_pad: bool) -> String {
    let bytes = input.as_bytes();
    match (url_safe, no_pad) {
        (false, false) => STANDARD.encode(bytes),
        (false, true) => STANDARD_NO_PAD.encode(bytes),
        (true, false) => URL_SAFE.encode(bytes),
        (true, true) => URL_SAFE_NO_PAD.encode(bytes),
    }
}

/// Base64 解码。宽容处理：自动忽略换行/空格，自动补 padding，自动识别 urlsafe 字符。
pub fn base64_decode(input: &str) -> Result<String> {
    let cleaned: String = input
        .chars()
        .filter(|c| !c.is_whitespace())
        .map(|c| match c {
            '-' => '+',
            '_' => '/',
            other => other,
        })
        .collect();

    if cleaned.is_empty() {
        return Err(IktError::Invalid("输入为空".into()));
    }

    let trimmed = cleaned.trim_end_matches('=');
    let mut padded = trimmed.to_string();
    let rem = padded.len() % 4;
    if rem == 1 {
        return Err(IktError::Invalid("不是合法的 Base64：长度异常".into()));
    }
    if rem > 0 {
        padded.push_str(&"=".repeat(4 - rem));
    }

    let raw = STANDARD
        .decode(padded.as_bytes())
        .map_err(|_| IktError::Invalid("不是合法的 Base64 文本".into()))?;

    String::from_utf8(raw).map_err(|_| {
        IktError::Invalid("解码得到的是二进制内容，无法按文本显示（可尝试十六进制视图）".into())
    })
}

pub fn hex_encode(input: &str) -> String {
    hex::encode(input.as_bytes())
}

pub fn hex_decode(input: &str) -> Result<String> {
    let cleaned: String = input.chars().filter(|c| !c.is_whitespace()).collect();
    let bytes = hex::decode(&cleaned).map_err(|_| IktError::Invalid("不是合法的十六进制".into()))?;
    String::from_utf8(bytes).map_err(|_| IktError::Invalid("解码得到的是二进制内容".into()))
}

/// URL 百分号编码保留哪些字符：`component` 对齐 encodeURIComponent，`uri` 对齐 encodeURI。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum UrlKeep {
    Component,
    Uri,
}

impl Default for UrlKeep {
    fn default() -> Self {
        UrlKeep::Component
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct UrlSpec {
    pub keep: UrlKeep,
    /// 空格编码成 `+`（表单 / application/x-www-form-urlencoded）
    pub plus_for_space: bool,
    /// `%e4` 还是 `%E4`
    pub upper_hex: bool,
}

impl Default for UrlSpec {
    fn default() -> Self {
        Self {
            keep: UrlKeep::Component,
            plus_for_space: false,
            upper_hex: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UrlOutcome {
    pub output: String,
    pub output_bytes: usize,
    /// 参与转换的 `%XX` 个数：编码时是产出的，解码时是还原的
    pub escapes: usize,
}

fn is_unreserved(b: u8) -> bool {
    b.is_ascii_alphanumeric()
        || matches!(b, b'-' | b'_' | b'.' | b'~' | b'!' | b'\'' | b'(' | b')' | b'*')
}

fn is_uri_delim(b: u8) -> bool {
    matches!(
        b,
        b';' | b'/' | b'?' | b':' | b'@' | b'&' | b'=' | b'+' | b'$' | b',' | b'#'
    )
}

fn hex_digit(n: u8, upper: bool) -> char {
    let c = if n < 10 { b'0' + n } else { b'a' + (n - 10) };
    if upper {
        (c as char).to_ascii_uppercase()
    } else {
        c as char
    }
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// URL 百分号编码（percent-encoding，无额外依赖的实现）。非 ASCII 走 UTF-8 逐字节转义。
pub fn url_encode(input: &str, spec: &UrlSpec) -> UrlOutcome {
    let mut out = String::with_capacity(input.len() * 2);
    let mut escapes = 0usize;
    for &b in input.as_bytes() {
        let keep = is_unreserved(b) || (spec.keep == UrlKeep::Uri && is_uri_delim(b));
        if keep {
            out.push(b as char);
        } else if b == b' ' && spec.plus_for_space {
            out.push('+');
        } else {
            out.push('%');
            out.push(hex_digit(b >> 4, spec.upper_hex));
            out.push(hex_digit(b & 0x0f, spec.upper_hex));
            escapes += 1;
        }
    }
    UrlOutcome {
        output_bytes: out.len(),
        output: out,
        escapes,
    }
}

pub fn url_decode(input: &str, spec: &UrlSpec) -> Result<UrlOutcome> {
    let bytes = input.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut escapes = 0usize;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' => {
                if i + 2 >= bytes.len() {
                    return Err(IktError::Invalid(format!(
                        "第 {} 个字符处的 % 转义不完整",
                        i + 1
                    )));
                }
                let hi = hex_val(bytes[i + 1]);
                let lo = hex_val(bytes[i + 2]);
                match (hi, lo) {
                    (Some(hi), Some(lo)) => {
                        out.push(hi * 16 + lo);
                        escapes += 1;
                        i += 3;
                    }
                    _ => {
                        return Err(IktError::Invalid(format!(
                            "第 {} 个字符处 % 后面不是两位十六进制",
                            i + 2
                        )))
                    }
                }
            }
            b'+' if spec.plus_for_space => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    let text =
        String::from_utf8(out).map_err(|_| IktError::Invalid("解码结果不是合法 UTF-8".into()))?;
    Ok(UrlOutcome {
        output_bytes: text.len(),
        output: text,
        escapes,
    })
}

/// UTF-8 文本转成纯 ASCII 时用的转义风格。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AsciiStyle {
    /// `\u4f60`（JSON / JS 字符串转义；非 BMP 字符拆成代理对）
    Js,
    /// `&#x4f60;`（HTML 十六进制实体）
    HtmlHex,
    /// `U+4F60`（Unicode 码点写法）
    CodePoint,
}

impl Default for AsciiStyle {
    fn default() -> Self {
        AsciiStyle::Js
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AsciiSpec {
    pub style: AsciiStyle,
    /// 十六进制字母大小写：`\u4F60` / `\u4f60`
    pub upper_hex: bool,
}

impl Default for AsciiSpec {
    fn default() -> Self {
        Self {
            style: AsciiStyle::Js,
            upper_hex: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AsciiOutcome {
    pub output: String,
    pub output_bytes: usize,
    /// 参与转换的字符 / 转义个数：编码时是转义的字符数，解码时是还原的转义数
    pub converted: usize,
    /// 超出 BMP、需要代理对的字符数
    pub astral: usize,
}

fn hex_pad(n: u32, width: usize, upper: bool) -> String {
    let s = format!("{:0width$x}", n, width = width);
    if upper {
        s.to_uppercase()
    } else {
        s
    }
}

/// 码点最小位宽：BMP 4 位，之后按实际位数，避免 `U+01F600` 这种错值。
fn cp_width(cp: u32) -> usize {
    if cp <= 0xffff {
        4
    } else if cp <= 0xf_ffff {
        5
    } else {
        6
    }
}

/// UTF-8 文本 → 纯 ASCII。ASCII 字符原样保留，不转义控制字符。
pub fn text_to_ascii(input: &str, spec: &AsciiSpec) -> AsciiOutcome {
    let mut out = String::with_capacity(input.len() + input.len() / 2);
    let mut converted = 0usize;
    let mut astral = 0usize;

    for ch in input.chars() {
        let cp = ch as u32;
        if cp < 0x80 {
            out.push(ch);
            continue;
        }
        converted += 1;
        if cp > 0xffff {
            astral += 1;
        }
        match spec.style {
            AsciiStyle::Js => {
                if cp <= 0xffff {
                    out.push_str("\\u");
                    out.push_str(&hex_pad(cp, 4, spec.upper_hex));
                } else {
                    let v = cp - 0x1_0000;
                    let hi = 0xd800 + (v >> 10);
                    let lo = 0xdc00 + (v & 0x3ff);
                    out.push_str("\\u");
                    out.push_str(&hex_pad(hi, 4, spec.upper_hex));
                    out.push_str("\\u");
                    out.push_str(&hex_pad(lo, 4, spec.upper_hex));
                }
            }
            AsciiStyle::HtmlHex => {
                out.push_str("&#x");
                out.push_str(&hex_pad(cp, cp_width(cp), spec.upper_hex));
                out.push(';');
            }
            AsciiStyle::CodePoint => {
                out.push_str("U+");
                out.push_str(&hex_pad(cp, cp_width(cp), spec.upper_hex));
            }
        }
    }

    AsciiOutcome {
        output_bytes: out.len(),
        output: out,
        converted,
        astral,
    }
}

fn scan_digits(chars: &[char], start: usize, max: usize, radix: u32) -> (u32, usize) {
    let mut val = 0u32;
    let mut n = 0usize;
    while n < max && start + n < chars.len() {
        match chars[start + n].to_digit(radix) {
            Some(d) => {
                val = val.saturating_mul(radix).saturating_add(d);
                n += 1;
            }
            None => break,
        }
    }
    (val, n)
}

fn invalid_pos(i: usize, msg: &str) -> IktError {
    IktError::Invalid(format!("第 {} 个字符处{}", i + 1, msg))
}

/// ASCII 转义 → UTF-8 文本。自动识别 `\uXXXX`、`\u{...}`、`&#x..;`、`&#..;`、`U+XXXX` 五种写法，
/// 认不出来的 `\` `&` `U` 按普通字符原样保留，不打断混合内容。
pub fn ascii_to_text(input: &str) -> Result<AsciiOutcome> {
    let chars: Vec<char> = input.chars().collect();
    let mut out = String::with_capacity(input.len());
    let mut converted = 0usize;
    let mut astral = 0usize;
    let mut i = 0usize;

    let push_cp = |out: &mut String, cp: u32, i: usize| -> Result<bool> {
        let ch = char::from_u32(cp)
            .ok_or_else(|| invalid_pos(i, "的码点不是合法字符（超出 Unicode 范围或为孤立代理项）"))?;
        out.push(ch);
        Ok(cp > 0xffff)
    };

    while i < chars.len() {
        let c = chars[i];

        // \uXXXX / \u{XXXXX}
        if c == '\\' && i + 1 < chars.len() && (chars[i + 1] == 'u' || chars[i + 1] == 'U') {
            let braced = i + 2 < chars.len() && chars[i + 2] == '{';
            let (cp, consumed) = if braced {
                let (val, n) = scan_digits(&chars, i + 3, 6, 16);
                match chars.get(i + 3 + n) {
                    Some('}') if n > 0 => (val, 3 + n + 1),
                    _ => (0, 0),
                }
            } else {
                let (val, n) = scan_digits(&chars, i + 2, 4, 16);
                if n == 4 {
                    (val, 2 + 4)
                } else {
                    (0, 0)
                }
            };

            if consumed > 0 {
                let mut total = consumed;
                let mut final_cp = cp;
                if (0xd800..=0xdbff).contains(&cp) {
                    // 高代理项：必须紧跟一个低代理项，拼成真实码点
                    let next = i + consumed;
                    let ok = chars.get(next) == Some(&'\\')
                        && chars.get(next + 1).map(|c| *c == 'u' || *c == 'U') == Some(true);
                    let (lo, n) = if ok { scan_digits(&chars, next + 2, 4, 16) } else { (0, 0) };
                    if !ok || n != 4 || !(0xdc00..=0xdfff).contains(&lo) {
                        return Err(invalid_pos(i, "是孤立的高代理项，后面缺少配对的 \\uDC00-\\uDFFF"));
                    }
                    final_cp = 0x1_0000 + ((cp - 0xd800) << 10) + (lo - 0xdc00);
                    total += 2 + 4;
                } else if (0xdc00..=0xdfff).contains(&cp) {
                    return Err(invalid_pos(i, "是孤立的低代理项，前面缺少配对的高代理项"));
                }

                if push_cp(&mut out, final_cp, i)? {
                    astral += 1;
                }
                converted += 1;
                i += total;
                continue;
            }
        }

        // &#xXXXX; / &#DDDD;
        if c == '&' && chars.get(i + 1) == Some(&'#') {
            let hex = matches!(chars.get(i + 2), Some('x') | Some('X'));
            let (start, radix, max) = if hex { (i + 3, 16, 6) } else { (i + 2, 10, 8) };
            let (val, n) = scan_digits(&chars, start, max, radix);
            if n > 0 && chars.get(start + n) == Some(&';') {
                if push_cp(&mut out, val, i)? {
                    astral += 1;
                }
                converted += 1;
                i = start + n + 1;
                continue;
            }
        }

        // U+XXXX
        if (c == 'U' || c == 'u') && chars.get(i + 1) == Some(&'+') {
            let (val, n) = scan_digits(&chars, i + 2, 6, 16);
            if n >= 4 {
                if push_cp(&mut out, val, i)? {
                    astral += 1;
                }
                converted += 1;
                i += 2 + n;
                continue;
            }
        }

        out.push(c);
        i += 1;
    }

    Ok(AsciiOutcome {
        output_bytes: out.len(),
        output: out,
        converted,
        astral,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_roundtrip() {
        let s = "iKunTools 开发者工具";
        let enc = base64_encode(s, false, false);
        assert_eq!(base64_decode(&enc).unwrap(), s);
    }

    #[test]
    fn base64_tolerates_missing_padding_and_newlines() {
        let enc = base64_encode("hello world", false, true);
        let wrapped = format!("{}\n  {}", &enc[..4], &enc[4..]);
        assert_eq!(base64_decode(&wrapped).unwrap(), "hello world");
    }

    #[test]
    fn base64_url_safe() {
        let enc = base64_encode("\u{fb}\u{ff}\u{fe}", true, true);
        assert!(!enc.contains('+') && !enc.contains('/') && !enc.contains('='));
        assert_eq!(base64_decode(&enc).unwrap(), "\u{fb}\u{ff}\u{fe}");
    }

    #[test]
    fn url_component_matches_encode_uri_component() {
        let spec = UrlSpec::default();
        // 与浏览器 encodeURIComponent 一致：! ' ( ) * 不转义
        assert_eq!(url_encode("a-b_c.d~e!f'g(h)i*j", &spec).output, "a-b_c.d~e!f'g(h)i*j");
        assert_eq!(url_encode("a b", &spec).output, "a%20b");
        assert_eq!(url_encode("/:?#[]@&=", &spec).output, "%2F%3A%3F%23%5B%5D%40%26%3D");
        assert_eq!(url_encode("中文", &spec).output, "%E4%B8%AD%E6%96%87");
    }

    #[test]
    fn url_uri_mode_keeps_delimiters() {
        let spec = UrlSpec {
            keep: UrlKeep::Uri,
            ..Default::default()
        };
        assert_eq!(
            url_encode("https://a.com/p?q=1&r=2#x", &spec).output,
            "https://a.com/p?q=1&r=2#x"
        );
        assert_eq!(url_encode("中文", &spec).output, "%E4%B8%AD%E6%96%87");
    }

    #[test]
    fn url_plus_and_hex_case() {
        let plus = UrlSpec {
            plus_for_space: true,
            ..Default::default()
        };
        assert_eq!(url_encode("a b", &plus).output, "a+b");

        let lower = UrlSpec {
            upper_hex: false,
            ..Default::default()
        };
        assert_eq!(url_encode("中", &lower).output, "%e4%b8%ad");

        assert_eq!(url_decode("a+b", &plus).unwrap().output, "a b");
        // 不解码 + 时原样保留
        assert_eq!(url_decode("a+b", &UrlSpec::default()).unwrap().output, "a+b");
    }

    #[test]
    fn url_roundtrip_and_escapes_count() {
        let spec = UrlSpec::default();
        let s = "a b+c/d?e=中文&f=1";
        let enc = url_encode(s, &spec);
        assert_eq!(enc.escapes, 13);
        assert_eq!(url_decode(&enc.output, &spec).unwrap().output, s);
    }

    #[test]
    fn url_decode_reports_position() {
        let spec = UrlSpec::default();
        let err = url_decode("abc%", &spec).unwrap_err().to_string();
        assert!(err.contains("第 4 个字符"), "{err}");
        let err = url_decode("ab%zz", &spec).unwrap_err().to_string();
        assert!(err.contains("第 4 个字符"), "{err}");
    }

    #[test]
    fn ascii_encode_styles() {
        let js = AsciiSpec::default();
        assert_eq!(text_to_ascii("中a文", &js).output, "\\u4e2da\\u6587");
        assert_eq!(text_to_ascii("中a文", &js).converted, 2);

        let upper = AsciiSpec {
            upper_hex: true,
            ..Default::default()
        };
        assert_eq!(text_to_ascii("中", &upper).output, "\\u4E2D");

        let html = AsciiSpec {
            style: AsciiStyle::HtmlHex,
            upper_hex: true,
            ..Default::default()
        };
        assert_eq!(text_to_ascii("中", &html).output, "&#x4E2D;");

        let cp = AsciiSpec {
            style: AsciiStyle::CodePoint,
            upper_hex: true,
            ..Default::default()
        };
        assert_eq!(text_to_ascii("中文", &cp).output, "U+4E2DU+6587");
    }

    #[test]
    fn ascii_encode_astral_uses_surrogate_pair() {
        let js = AsciiSpec::default();
        let out = text_to_ascii("😀", &js);
        assert_eq!(out.output, "\\ud83d\\ude00");
        assert_eq!(out.astral, 1);
        assert_eq!(out.output_bytes, 12);

        let html = AsciiSpec {
            style: AsciiStyle::HtmlHex,
            ..Default::default()
        };
        assert_eq!(text_to_ascii("😀", &html).output, "&#x1f600;");

        let cp = AsciiSpec {
            style: AsciiStyle::CodePoint,
            upper_hex: true,
            ..Default::default()
        };
        assert_eq!(text_to_ascii("😀", &cp).output, "U+1F600");
    }

    #[test]
    fn ascii_decode_mixed_forms() {
        let src = "\\u4e2d&#x6587;U+0041&#20320;\\u{1F600}";
        let out = ascii_to_text(src).unwrap();
        assert_eq!(out.output, "中文A你😀");
        assert_eq!(out.converted, 5);
        assert_eq!(out.astral, 1);
    }

    #[test]
    fn ascii_decode_roundtrip_all_styles() {
        let text = "iKunTools · 开发者工具 🚀 mixed ASCII";
        for style in [AsciiStyle::Js, AsciiStyle::HtmlHex, AsciiStyle::CodePoint] {
            let spec = AsciiSpec {
                style,
                upper_hex: true,
            };
            let enc = text_to_ascii(text, &spec);
            assert!(enc.output.is_ascii(), "{:?} 输出不是纯 ASCII", style);
            assert_eq!(ascii_to_text(&enc.output).unwrap().output, text, "{:?}", style);
        }
    }

    #[test]
    fn ascii_decode_keeps_plain_text() {
        // C:\users 这种反斜杠不当转义，U+ 后面不足 4 位也原样保留
        assert_eq!(ascii_to_text("C:\\users\\n").unwrap().output, "C:\\users\\n");
        assert_eq!(ascii_to_text("a U+1 b").unwrap().output, "a U+1 b");
        assert_eq!(ascii_to_text("&# nope").unwrap().output, "&# nope");
        assert_eq!(ascii_to_text("字节").unwrap().converted, 0);
    }

    #[test]
    fn ascii_decode_rejects_broken_surrogates() {
        let err = ascii_to_text("ab\\ud83d").unwrap_err().to_string();
        assert!(err.contains("第 3 个字符"), "{err}");
        let err = ascii_to_text("\\ude00").unwrap_err().to_string();
        assert!(err.contains("低代理项"), "{err}");
        let err = ascii_to_text("\\u{110000}").unwrap_err().to_string();
        assert!(err.contains("超出 Unicode 范围"), "{err}");
    }

    #[test]
    fn ascii_contract_is_camel_case() {
        let spec = serde_json::to_value(AsciiSpec::default()).unwrap();
        for key in ["style", "upperHex"] {
            assert!(spec.get(key).is_some(), "缺字段 {key}");
        }
        assert_eq!(spec["style"], "js");

        let out = serde_json::to_value(text_to_ascii("中", &AsciiSpec::default())).unwrap();
        for key in ["output", "outputBytes", "converted", "astral"] {
            assert!(out.get(key).is_some(), "缺字段 {key}");
        }
        assert_eq!(serde_json::to_value(AsciiStyle::HtmlHex).unwrap(), "htmlHex");
        assert_eq!(serde_json::to_value(AsciiStyle::CodePoint).unwrap(), "codePoint");
    }

    #[test]
    fn url_contract_is_camel_case() {
        let spec = serde_json::to_value(UrlSpec::default()).unwrap();
        for key in ["keep", "plusForSpace", "upperHex"] {
            assert!(spec.get(key).is_some(), "缺字段 {key}");
        }
        assert_eq!(spec["keep"], "component");

        let out = serde_json::to_value(url_encode("中", &UrlSpec::default())).unwrap();
        for key in ["output", "outputBytes", "escapes"] {
            assert!(out.get(key).is_some(), "缺字段 {key}");
        }
    }
}
