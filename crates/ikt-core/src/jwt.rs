use base64::Engine;
use serde::Serialize;
use serde_json::Value;

use crate::{IktError, Result};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JwtParts {
    pub algorithm: String,
    pub typ: String,
    pub header: String,
    pub payload: String,
    pub signature: String,
    /// 注册声明里的时间字段（秒级时间戳）
    pub iat: Option<i64>,
    pub exp: Option<i64>,
    pub nbf: Option<i64>,
    pub iat_text: Option<String>,
    pub exp_text: Option<String>,
    pub nbf_text: Option<String>,
    /// 与当前时间比较的结果
    pub expired: Option<bool>,
    pub seconds_left: Option<i64>,
    pub missing_signature: bool,
}

fn b64url_decode(part: &str) -> Result<Vec<u8>> {
    let cleaned: String = part.chars().filter(|c| !c.is_whitespace()).collect();
    let trimmed = cleaned.trim_end_matches('=');
    // base64url 与标准 base64 只差两个字符，统一换回标准字母表再解码
    let mut s = trimmed.replace('-', "+").replace('_', "/");
    let rem = s.len() % 4;
    if rem == 1 {
        return Err(IktError::Invalid("JWT 段长度异常".into()));
    }
    if rem > 0 {
        s.push_str(&"=".repeat(4 - rem));
    }
    base64::engine::general_purpose::STANDARD
        .decode(s.as_bytes())
        .map_err(|_| IktError::Invalid("JWT 段不是合法的 base64url".into()))
}

fn pretty(bytes: &[u8]) -> Result<(String, Value)> {
    let text = String::from_utf8(bytes.to_vec())
        .map_err(|_| IktError::Invalid("JWT 段不是 UTF-8 文本".into()))?;
    match serde_json::from_str::<Value>(&text) {
        Ok(v) => {
            let formatted = serde_json::to_string_pretty(&v).unwrap_or_else(|_| text.clone());
            Ok((formatted, v))
        }
        Err(_) => Ok((text, Value::Null)),
    }
}

fn ts_text(sec: i64) -> Option<String> {
    chrono::DateTime::from_timestamp(sec, 0).map(|dt| {
        dt.with_timezone(&chrono::Local)
            .format("%Y-%m-%d %H:%M:%S %:z")
            .to_string()
    })
}

/// 仅解码，不做签名验证。工具定位是「看清楚里面写了什么」，验签交给后端。
pub fn decode(token: &str) -> Result<JwtParts> {
    let token = token.trim().trim_start_matches("Bearer ").trim();
    if token.is_empty() {
        return Err(IktError::Invalid("请粘贴 JWT".into()));
    }
    let segments: Vec<&str> = token.split('.').collect();
    if segments.len() < 2 {
        return Err(IktError::Invalid("JWT 至少需要 header.payload 两段".into()));
    }

    let (header, header_val) = pretty(&b64url_decode(segments[0])?)?;
    let (payload, payload_val) = pretty(&b64url_decode(segments[1])?)?;
    let signature = segments.get(2).map(|s| s.to_string()).unwrap_or_default();

    let algorithm = header_val
        .get("alg")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();
    let typ = header_val
        .get("typ")
        .and_then(|v| v.as_str())
        .unwrap_or("JWT")
        .to_string();

    let num = |k: &str| payload_val.get(k).and_then(|v| v.as_i64());
    let iat = num("iat");
    let exp = num("exp");
    let nbf = num("nbf");
    let now = chrono::Utc::now().timestamp();
    let missing_signature = signature.is_empty();

    Ok(JwtParts {
        algorithm,
        typ,
        header,
        payload,
        signature,
        iat,
        exp,
        nbf,
        iat_text: iat.and_then(ts_text),
        exp_text: exp.and_then(ts_text),
        nbf_text: nbf.and_then(ts_text),
        expired: exp.map(|e| e < now),
        seconds_left: exp.map(|e| e - now),
        missing_signature,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // HS256 示例 token（签名不重要，只验证解析）
    const SAMPLE: &str = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IuW8oOS4iSIsImlhdCI6MTcwMDAwMDAwMCwiZXhwIjo0MTAyNDQ0ODAwfQ.dummy-signature";

    #[test]
    fn decodes_header_and_payload() {
        let p = decode(SAMPLE).unwrap();
        assert_eq!(p.algorithm, "HS256");
        assert!(p.header.contains("HS256"));
        assert!(p.payload.contains("张三"));
        assert_eq!(p.iat, Some(1_700_000_000));
        assert_eq!(p.expired, Some(false));
        assert!(p.exp_text.unwrap().contains('-'));
    }

    #[test]
    fn tolerates_bearer_prefix() {
        let p = decode(&format!("Bearer {SAMPLE}")).unwrap();
        assert_eq!(p.typ, "JWT");
    }

    #[test]
    fn rejects_garbage() {
        assert!(decode("not-a-token").is_err());
    }

    /// 前后端契约：字段必须是 camelCase，改名必须同步改 src/lib/api.ts
    #[test]
    fn serializes_camel_case_contract() {
        let v = serde_json::to_value(decode(SAMPLE).unwrap()).unwrap();
        for key in [
            "algorithm",
            "typ",
            "header",
            "payload",
            "signature",
            "iatText",
            "expText",
            "secondsLeft",
            "missingSignature",
            "expired",
        ] {
            assert!(v.get(key).is_some(), "缺少字段 {key}");
        }
    }
}
