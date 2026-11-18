//! HTTP 请求：把界面上的请求草稿组装成确定的报文，再发出去。
//!
//! `build` 与 `send` 共用同一套组装逻辑，所以界面上展示的 cURL 命令
//! 与实际发出的请求逐字节一致。组装是纯函数，发送才碰 IO。

use std::time::{Duration, Instant};

use base64::{engine::general_purpose::STANDARD as B64, Engine};
use serde::{Deserialize, Serialize};

use crate::encoding::{url_encode, UrlKeep, UrlSpec};
use crate::{IktError, Result};

const METHODS: [&str; 8] = ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS", "TRACE"];
const MAX_TIMEOUT_MS: u64 = 300_000;
const MAX_BODY_LIMIT: u64 = 64 * 1024 * 1024;
const UA: &str = "iKunTools/0.1";

fn yes() -> bool {
    true
}

/// 请求头 / 查询参数 / 表单字段共用的键值对。`enabled=false` 的行不参与组装。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyValue {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub value: String,
    #[serde(default = "yes")]
    pub enabled: bool,
}

impl KeyValue {
    fn new(name: &str, value: impl Into<String>) -> Self {
        Self {
            name: name.to_string(),
            value: value.into(),
            enabled: true,
        }
    }

    fn usable(&self) -> bool {
        self.enabled && !self.name.trim().is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct HttpOptions {
    pub timeout_ms: u64,
    pub follow_redirects: bool,
    pub max_redirects: u32,
    pub verify_tls: bool,
    /// 最多读回多少字节的响应体，超出的部分丢弃并标记 truncated
    pub max_body: u64,
    /// 走代理（HTTP CONNECT）
    pub proxy_enabled: bool,
    /// `http://127.0.0.1:8080`、`127.0.0.1:8080` 都可，允许自带 user:pass@
    pub proxy_url: String,
    /// 单独填的凭据，优先于地址里自带的
    pub proxy_user: String,
    pub proxy_pass: String,
}

impl Default for HttpOptions {
    fn default() -> Self {
        Self {
            timeout_ms: 30_000,
            follow_redirects: true,
            max_redirects: 10,
            verify_tls: true,
            max_body: 5 * 1024 * 1024,
            proxy_enabled: false,
            proxy_url: String::new(),
            proxy_user: String::new(),
            proxy_pass: String::new(),
        }
    }
}

/// 界面上的完整请求草稿。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct HttpSpec {
    pub method: String,
    pub url: String,
    pub params: Vec<KeyValue>,
    pub headers: Vec<KeyValue>,
    /// none / raw / urlencoded / form
    pub body_kind: String,
    /// raw 的正文
    pub body: String,
    /// urlencoded / form 的字段
    pub body_params: Vec<KeyValue>,
    /// raw 的 Content-Type，留空按内容猜
    pub content_type: String,
    /// none / bearer / basic
    pub auth_kind: String,
    pub auth_token: String,
    pub auth_user: String,
    pub auth_pass: String,
    pub options: HttpOptions,
}

/// 组装结果：cURL 命令里出现的东西都在这里，和真正发出去的一模一样。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuiltRequest {
    pub method: String,
    pub url: String,
    pub headers: Vec<KeyValue>,
    /// 正文原文。multipart 时已经带好边界，可直接当文本看。
    pub body: String,
    pub body_bytes: usize,
    pub content_type: Option<String>,
    pub multipart: bool,
    pub form_fields: Vec<KeyValue>,
    /// 归一化后的代理地址（含凭据），cURL 的 -x 直接用它
    pub proxy: Option<String>,
    pub host: String,
    pub has_body: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpOutcome {
    pub status: u16,
    pub status_text: String,
    pub http_version: String,
    pub headers: Vec<KeyValue>,
    /// 文本响应是原文；二进制响应是 Base64
    pub body: String,
    pub is_base64: bool,
    pub body_bytes: u64,
    pub declared_bytes: Option<u64>,
    pub truncated: bool,
    pub elapsed_ms: f64,
    pub url: String,
    pub content_type: String,
}

fn normalize_method(raw: &str) -> Result<String> {
    let m = raw.trim().to_ascii_uppercase();
    if m.is_empty() {
        return Ok("GET".into());
    }
    if !METHODS.contains(&m.as_str()) {
        return Err(IktError::Invalid(format!("不支持的请求方法：{m}")));
    }
    Ok(m)
}

fn component(input: &str) -> String {
    url_encode(
        input,
        &UrlSpec {
            keep: UrlKeep::Component,
            plus_for_space: false,
            upper_hex: true,
        },
    )
    .output
}

fn form_value(input: &str) -> String {
    url_encode(
        input,
        &UrlSpec {
            keep: UrlKeep::Component,
            plus_for_space: true,
            upper_hex: true,
        },
    )
    .output
}

fn query_string(params: &[KeyValue]) -> String {
    params
        .iter()
        .filter(|p| p.usable())
        .map(|p| format!("{}={}", component(p.name.trim()), component(&p.value)))
        .collect::<Vec<_>>()
        .join("&")
}

/// 直接把不可见字符和非 ASCII 字节转成 %XX（已有的 %XX 原样保留）。
fn encode_literal(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len() * 2);
    for b in raw.bytes() {
        if b <= 0x20 || b >= 0x7f {
            out.push_str(&format!("%{b:02X}"));
        } else {
            out.push(b as char);
        }
    }
    out
}

fn compose_url(raw: &str, params: &[KeyValue]) -> Result<String> {
    let mut url = raw.trim().to_string();
    if url.is_empty() {
        return Err(IktError::Invalid("请填写 URL".into()));
    }
    // 空格、中文这些不合法字符按 UTF-8 逐字节转义（Postman 也是这么干的）
    url = encode_literal(&url);
    if !url.contains("://") {
        url = format!("http://{url}");
    }
    let lower = url.to_ascii_lowercase();
    if !lower.starts_with("http://") && !lower.starts_with("https://") {
        return Err(IktError::Invalid("只支持 http / https 地址".into()));
    }

    let qs = query_string(params);
    if !qs.is_empty() {
        // 插在 #fragment 之前，且不要和已有查询串粘在一起
        let (head, tail) = match url.find('#') {
            Some(i) => (url[..i].to_string(), url[i..].to_string()),
            None => (url.clone(), String::new()),
        };
        let sep = if head.contains('?') {
            if head.ends_with('?') || head.ends_with('&') {
                ""
            } else {
                "&"
            }
        } else {
            "?"
        };
        url = format!("{head}{sep}{qs}{tail}");
    }

    if url.parse::<ureq::http::Uri>().is_err() {
        return Err(IktError::Invalid("URL 格式不正确".into()));
    }
    Ok(url)
}

/// 归一化代理地址：`127.0.0.1:8080` → `http://127.0.0.1:8080`；单独填的凭据会转义后拼进 userinfo。
fn compose_proxy(opts: &HttpOptions) -> Result<Option<String>> {
    if !opts.proxy_enabled {
        return Ok(None);
    }
    let raw = opts.proxy_url.trim();
    if raw.is_empty() {
        return Err(IktError::Invalid("请填写代理地址".into()));
    }
    let with_scheme = if raw.contains("://") {
        raw.to_string()
    } else {
        format!("http://{raw}")
    };
    let (scheme, rest) = with_scheme
        .split_once("://")
        .ok_or_else(|| IktError::Invalid("代理地址格式不正确".into()))?;
    let scheme = scheme.to_ascii_lowercase();
    // socks 需要额外 feature，这里只支持 HTTP CONNECT
    if scheme != "http" && scheme != "https" {
        return Err(IktError::Invalid("只支持 http / https 代理".into()));
    }
    let authority = rest.rsplit('@').next().unwrap_or(rest);
    if authority.is_empty() {
        return Err(IktError::Invalid("代理地址缺少主机".into()));
    }

    let url = if opts.proxy_user.is_empty() && opts.proxy_pass.is_empty() {
        format!("{scheme}://{rest}")
    } else {
        format!(
            "{scheme}://{}:{}@{authority}",
            component(&opts.proxy_user),
            component(&opts.proxy_pass)
        )
    };
    if url.parse::<ureq::http::Uri>().is_err() {
        return Err(IktError::Invalid("代理地址格式不正确".into()));
    }
    Ok(Some(url))
}

fn guess_content_type(body: &str) -> &'static str {
    let head = body.trim_start();
    if head.starts_with('{') || head.starts_with('[') {
        "application/json"
    } else if head.starts_with('<') {
        "application/xml"
    } else {
        "text/plain"
    }
}

fn compose_body(spec: &HttpSpec) -> (String, Option<String>, bool, Vec<KeyValue>) {
    let fields: Vec<KeyValue> = spec
        .body_params
        .iter()
        .filter(|p| p.usable())
        .cloned()
        .collect();

    match spec.body_kind.as_str() {
        "raw" => {
            let ct = spec.content_type.trim();
            let ct = if ct.is_empty() {
                guess_content_type(&spec.body).to_string()
            } else {
                ct.to_string()
            };
            (spec.body.clone(), Some(ct), false, Vec::new())
        }
        "urlencoded" => {
            let body = fields
                .iter()
                .map(|f| format!("{}={}", form_value(f.name.trim()), form_value(&f.value)))
                .collect::<Vec<_>>()
                .join("&");
            (
                body,
                Some("application/x-www-form-urlencoded".into()),
                false,
                fields,
            )
        }
        "form" => {
            let boundary = format!("----iKunToolsBoundary{:016x}", rand::random::<u64>());
            let mut body = String::new();
            for f in &fields {
                body.push_str(&format!("--{boundary}\r\n"));
                body.push_str(&format!(
                    "Content-Disposition: form-data; name=\"{}\"\r\n\r\n",
                    f.name.trim().replace('"', "%22")
                ));
                body.push_str(&f.value);
                body.push_str("\r\n");
            }
            body.push_str(&format!("--{boundary}--\r\n"));
            (
                body,
                Some(format!("multipart/form-data; boundary={boundary}")),
                true,
                fields,
            )
        }
        _ => (String::new(), None, false, Vec::new()),
    }
}

/// 组装请求。纯函数，不发包，方便界面实时预览 cURL。
pub fn build(spec: &HttpSpec) -> Result<BuiltRequest> {
    let method = normalize_method(&spec.method)?;
    let url = compose_url(&spec.url, &spec.params)?;
    let (body, content_type, multipart, form_fields) = compose_body(spec);

    let mut headers: Vec<KeyValue> = spec
        .headers
        .iter()
        .filter(|h| h.usable())
        .map(|h| KeyValue::new(h.name.trim(), h.value.clone()))
        .collect();

    let has = |list: &[KeyValue], name: &str| list.iter().any(|h| h.name.eq_ignore_ascii_case(name));

    // 认证：已手写了 Authorization 就以手写的为准
    if !has(&headers, "authorization") {
        match spec.auth_kind.as_str() {
            "bearer" if !spec.auth_token.trim().is_empty() => headers.push(KeyValue::new(
                "Authorization",
                format!("Bearer {}", spec.auth_token.trim()),
            )),
            "basic" => {
                let raw = format!("{}:{}", spec.auth_user, spec.auth_pass);
                headers.push(KeyValue::new(
                    "Authorization",
                    format!("Basic {}", B64.encode(raw.as_bytes())),
                ));
            }
            _ => {}
        }
    }

    if let Some(ct) = &content_type {
        if !has(&headers, "content-type") {
            headers.push(KeyValue::new("Content-Type", ct.clone()));
        }
    }
    if !has(&headers, "user-agent") {
        headers.push(KeyValue::new("User-Agent", UA));
    }
    if !has(&headers, "accept") {
        headers.push(KeyValue::new("Accept", "*/*"));
    }

    // 这两个由客户端按实际报文重算，手填只会出错
    headers.retain(|h| {
        !h.name.eq_ignore_ascii_case("host") && !h.name.eq_ignore_ascii_case("content-length")
    });

    let proxy = compose_proxy(&spec.options)?;

    let uri = url
        .parse::<ureq::http::Uri>()
        .map_err(|_| IktError::Invalid("URL 格式不正确".into()))?;
    let host = uri
        .authority()
        .map(|a| a.to_string())
        .unwrap_or_else(String::new);

    Ok(BuiltRequest {
        has_body: spec.body_kind != "none" && !spec.body_kind.is_empty(),
        method,
        url,
        body_bytes: body.len(),
        headers,
        body,
        content_type,
        multipart,
        form_fields,
        proxy,
        host,
    })
}

fn header_value(headers: &[KeyValue], name: &str) -> Option<String> {
    headers
        .iter()
        .find(|h| h.name.eq_ignore_ascii_case(name))
        .map(|h| h.value.clone())
}

fn is_textual(content_type: &str) -> bool {
    if content_type.is_empty() {
        return true;
    }
    let ct = content_type.to_ascii_lowercase();
    ct.starts_with("text/")
        || ct.contains("json")
        || ct.contains("xml")
        || ct.contains("javascript")
        || ct.contains("ecmascript")
        || ct.contains("x-www-form-urlencoded")
        || ct.contains("graphql")
        || ct.contains("csv")
        || ct.contains("yaml")
        || ct.contains("x-ndjson")
}

fn map_error(e: ureq::Error) -> IktError {
    let msg = match e {
        ureq::Error::Timeout(_) => "请求超时".to_string(),
        ureq::Error::BadUri(_) => "URL 格式不正确".to_string(),
        ureq::Error::HostNotFound => "域名解析失败".to_string(),
        ureq::Error::ConnectionFailed => "无法建立连接".to_string(),
        other => other.to_string(),
    };
    IktError::Invalid(format!("请求失败：{msg}"))
}

/// 真正发包。阻塞调用，调用方负责丢到后台线程。
pub fn send(spec: &HttpSpec) -> Result<HttpOutcome> {
    let built = build(spec)?;
    let opts = &spec.options;

    let tls = ureq::tls::TlsConfig::builder()
        .disable_verification(!opts.verify_tls)
        .build();

    let proxy = match &built.proxy {
        Some(url) => Some(
            ureq::Proxy::new(url)
                .map_err(|e| IktError::Invalid(format!("代理地址不合法：{e}")))?,
        ),
        None => None,
    };

    let config = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(Duration::from_millis(
            opts.timeout_ms.clamp(1, MAX_TIMEOUT_MS),
        )))
        .max_redirects(if opts.follow_redirects {
            opts.max_redirects.min(20)
        } else {
            0
        })
        .tls_config(tls)
        .proxy(proxy)
        .user_agent(UA)
        .build();
    let agent = ureq::Agent::new_with_config(config);

    let method = ureq::http::Method::from_bytes(built.method.as_bytes())
        .map_err(|_| IktError::Invalid("请求方法不合法".into()))?;
    let mut builder = ureq::http::Request::builder()
        .method(method)
        .uri(built.url.as_str());
    for h in &built.headers {
        builder = builder.header(h.name.as_str(), h.value.as_str());
    }
    let request = builder
        .body(built.body.clone().into_bytes())
        .map_err(|e| IktError::Invalid(format!("请求头不合法：{e}")))?;

    let started = Instant::now();
    let mut resp = agent.run(request).map_err(map_error)?;
    let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;

    let status = resp.status();
    let status_text = status.canonical_reason().unwrap_or("").to_string();
    let http_version = format!("{:?}", resp.version());

    let mut headers = Vec::new();
    for (name, value) in resp.headers() {
        headers.push(KeyValue {
            name: name.as_str().to_string(),
            value: String::from_utf8_lossy(value.as_bytes()).to_string(),
            enabled: true,
        });
    }
    let content_type = header_value(&headers, "content-type").unwrap_or_default();
    let declared_bytes = header_value(&headers, "content-length")
        .and_then(|v| v.trim().parse::<u64>().ok());

    let limit = opts.max_body.clamp(1, MAX_BODY_LIMIT);
    let bytes = resp
        .body_mut()
        .with_config()
        .limit(limit)
        .read_to_vec()
        .map_err(map_error)?;

    let read = bytes.len() as u64;
    let truncated = declared_bytes.map(|d| d > read).unwrap_or(read >= limit);

    let (body, is_base64) = if is_textual(&content_type) {
        match String::from_utf8(bytes) {
            Ok(text) => (text, false),
            // 声明是文本但字节不是 UTF-8，退回 Base64，别显示乱码
            Err(e) => (B64.encode(e.into_bytes()), true),
        }
    } else {
        (B64.encode(bytes), true)
    };

    Ok(HttpOutcome {
        status: status.as_u16(),
        status_text,
        http_version,
        headers,
        body,
        is_base64,
        body_bytes: read,
        declared_bytes,
        truncated,
        elapsed_ms,
        url: built.url,
        content_type,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    fn spec(method: &str, url: &str) -> HttpSpec {
        HttpSpec {
            method: method.into(),
            url: url.into(),
            ..Default::default()
        }
    }

    #[test]
    fn build_adds_scheme_and_keeps_path() {
        let b = build(&spec("get", "example.com/a/b?x=1")).unwrap();
        assert_eq!(b.method, "GET");
        assert_eq!(b.url, "http://example.com/a/b?x=1");
        assert_eq!(b.host, "example.com");
        assert!(!b.has_body);
    }

    #[test]
    fn build_appends_encoded_query_before_fragment() {
        let mut s = spec("GET", "https://api.test/search?page=1#top");
        s.params = vec![
            KeyValue::new("q", "hello world"),
            KeyValue::new("tag", "中文&x"),
            KeyValue {
                name: "off".into(),
                value: "1".into(),
                enabled: false,
            },
            KeyValue::new("", "ignored"),
        ];
        let b = build(&s).unwrap();
        assert_eq!(
            b.url,
            "https://api.test/search?page=1&q=hello%20world&tag=%E4%B8%AD%E6%96%87%26x#top"
        );
        assert_eq!(b.url.matches('?').count(), 1);
    }

    #[test]
    fn build_injects_bearer_and_basic_auth() {
        let mut s = spec("GET", "https://api.test/me");
        s.auth_kind = "bearer".into();
        s.auth_token = "abc.def".into();
        let b = build(&s).unwrap();
        assert_eq!(
            header_value(&b.headers, "authorization").unwrap(),
            "Bearer abc.def"
        );

        s.auth_kind = "basic".into();
        s.auth_user = "kun".into();
        s.auth_pass = "123".into();
        let b = build(&s).unwrap();
        assert_eq!(
            header_value(&b.headers, "authorization").unwrap(),
            "Basic a3VuOjEyMw=="
        );

        // 手写的 Authorization 优先
        s.headers = vec![KeyValue::new("authorization", "Token raw")];
        let b = build(&s).unwrap();
        assert_eq!(
            header_value(&b.headers, "authorization").unwrap(),
            "Token raw"
        );
        assert_eq!(
            b.headers
                .iter()
                .filter(|h| h.name.eq_ignore_ascii_case("authorization"))
                .count(),
            1
        );
    }

    #[test]
    fn build_guesses_content_type_for_raw_body() {
        let mut s = spec("POST", "https://api.test/x");
        s.body_kind = "raw".into();
        s.body = r#"{"a":1}"#.into();
        let b = build(&s).unwrap();
        assert_eq!(b.content_type.as_deref(), Some("application/json"));
        assert_eq!(header_value(&b.headers, "content-type").unwrap(), "application/json");
        assert!(b.has_body);
        assert_eq!(b.body_bytes, 7);

        // 自定义 Content-Type 覆盖猜测
        s.content_type = "application/vnd.api+json".into();
        let b = build(&s).unwrap();
        assert_eq!(b.content_type.as_deref(), Some("application/vnd.api+json"));

        // 手写 header 优先于自动补的
        s.headers = vec![KeyValue::new("Content-Type", "text/plain")];
        let b = build(&s).unwrap();
        assert_eq!(
            b.headers
                .iter()
                .filter(|h| h.name.eq_ignore_ascii_case("content-type"))
                .count(),
            1
        );
    }

    #[test]
    fn build_urlencoded_and_multipart_bodies() {
        let mut s = spec("POST", "https://api.test/form");
        s.body_kind = "urlencoded".into();
        s.body_params = vec![KeyValue::new("name", "i kun"), KeyValue::new("n", "1")];
        let b = build(&s).unwrap();
        assert_eq!(b.body, "name=i+kun&n=1");
        assert_eq!(
            b.content_type.as_deref(),
            Some("application/x-www-form-urlencoded")
        );
        assert_eq!(b.form_fields.len(), 2);

        s.body_kind = "form".into();
        let b = build(&s).unwrap();
        let ct = b.content_type.clone().unwrap();
        assert!(ct.starts_with("multipart/form-data; boundary="));
        assert!(b.multipart);
        assert!(b.body.contains("Content-Disposition: form-data; name=\"name\""));
        assert!(b.body.ends_with("--\r\n"));
        let boundary = ct.rsplit("boundary=").next().unwrap();
        assert!(b.body.starts_with(&format!("--{boundary}\r\n")));
    }

    #[test]
    fn build_drops_host_header_and_rejects_bad_url() {
        let mut s = spec("GET", "https://api.test/x");
        s.headers = vec![
            KeyValue::new("Host", "evil.test"),
            KeyValue::new("X-Trace", "1"),
        ];
        let b = build(&s).unwrap();
        assert!(header_value(&b.headers, "host").is_none());
        assert_eq!(header_value(&b.headers, "x-trace").unwrap(), "1");

        assert!(build(&spec("GET", "")).is_err());
        assert!(build(&spec("GET", "ftp://api.test/x")).is_err());
        assert!(build(&spec("FETCH", "https://api.test")).is_err());

        // 空格 / 中文自动转义，已经转义过的 %XX 不动
        let b = build(&spec("GET", "https://api.test/中文 ok?q=%20")).unwrap();
        assert_eq!(b.url, "https://api.test/%E4%B8%AD%E6%96%87%20ok?q=%20");
    }

    #[test]
    fn build_normalizes_proxy() {
        let mut s = spec("GET", "https://api.test/x");
        s.options.proxy_enabled = true;

        s.options.proxy_url = "127.0.0.1:8080".into();
        assert_eq!(
            build(&s).unwrap().proxy.as_deref(),
            Some("http://127.0.0.1:8080")
        );

        // 地址里自带凭据时原样保留
        s.options.proxy_url = "https://u:p@proxy.test:1080".into();
        assert_eq!(
            build(&s).unwrap().proxy.as_deref(),
            Some("https://u:p@proxy.test:1080")
        );

        // 单独填的凭据优先，特殊字符要转义
        s.options.proxy_url = "proxy.test:8080".into();
        s.options.proxy_user = "kun".into();
        s.options.proxy_pass = "p@ss:1".into();
        let url = build(&s).unwrap().proxy.unwrap();
        assert_eq!(url, "http://kun:p%40ss%3A1@proxy.test:8080");
        assert!(ureq::Proxy::new(&url).is_ok(), "ureq 解析不了：{url}");

        // 关掉就完全不管代理地址
        s.options.proxy_enabled = false;
        s.options.proxy_url = ":::bad".into();
        assert!(build(&s).unwrap().proxy.is_none());

        // 开着但没填 / 协议不支持 → 报错
        s.options.proxy_enabled = true;
        s.options.proxy_url = String::new();
        assert!(build(&s).is_err());
        s.options.proxy_url = "socks5://127.0.0.1:1080".into();
        assert!(build(&s).is_err());
    }

    #[test]
    fn send_goes_through_http_proxy() {
        let proxy = TcpListener::bind("127.0.0.1:0").unwrap();
        let paddr = proxy.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = proxy.accept().unwrap();
            let mut raw = Vec::new();
            let mut buf = [0u8; 1024];
            loop {
                let n = stream.read(&mut buf).unwrap();
                raw.extend_from_slice(&buf[..n]);
                if raw.windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }
            let head = String::from_utf8_lossy(&raw).to_string();
            stream
                .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
                .unwrap();
            let n = stream.read(&mut buf).unwrap();
            let inner = String::from_utf8_lossy(&buf[..n]).to_string();
            let body = "via-proxy";
            stream
                .write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    )
                    .as_bytes(),
                )
                .unwrap();
            (head, inner)
        });

        // 这个域名根本不解析，能拿到响应就说明确实走了代理
        let mut s = spec("GET", "http://unreachable.test/probe");
        s.options.proxy_enabled = true;
        s.options.proxy_url = paddr.to_string();
        s.options.timeout_ms = 5_000;

        let out = send(&s).unwrap();
        let (head, inner) = server.join().unwrap();
        assert!(head.starts_with("CONNECT unreachable.test:80"), "{head}");
        assert!(inner.starts_with("GET /probe HTTP/1.1"), "{inner}");
        assert_eq!(out.body, "via-proxy");
    }

    #[test]
    fn serializes_camel_case_contract() {
        let mut s = spec("POST", "https://api.test/x");
        s.body_kind = "raw".into();
        s.body_params = vec![KeyValue::new("a", "b")];
        let v = serde_json::to_value(&s).unwrap();
        for key in [
            "bodyKind",
            "bodyParams",
            "contentType",
            "authKind",
            "authToken",
            "authUser",
            "authPass",
        ] {
            assert!(v.get(key).is_some(), "缺字段 {key}");
        }
        assert!(v["options"].get("followRedirects").is_some());
        assert!(v["options"].get("maxRedirects").is_some());
        assert!(v["options"].get("verifyTls").is_some());
        assert!(v["options"].get("maxBody").is_some());
        assert!(v["options"].get("proxyEnabled").is_some());
        assert!(v["options"].get("proxyUrl").is_some());
        assert!(v["options"].get("proxyUser").is_some());
        assert!(v["options"].get("proxyPass").is_some());

        let parsed: HttpSpec = serde_json::from_value(serde_json::json!({
            "method": "GET",
            "url": "https://api.test",
            "headers": [{"name": "a", "value": "b"}],
        }))
        .unwrap();
        assert_eq!(parsed.headers.len(), 1);
        assert!(parsed.headers[0].enabled, "enabled 缺省应为 true");
        assert_eq!(parsed.options.timeout_ms, 30_000);
        assert!(parsed.options.verify_tls);

        let b = build(&parsed).unwrap();
        let v = serde_json::to_value(&b).unwrap();
        for key in ["bodyBytes", "contentType", "formFields", "hasBody"] {
            assert!(v.get(key).is_some(), "缺字段 {key}");
        }
    }

    #[test]
    fn send_against_local_server() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            // 头和体可能分两个 TCP 段到达，按 Content-Length 读全
            let mut raw: Vec<u8> = Vec::new();
            let mut buf = [0u8; 1024];
            loop {
                let n = stream.read(&mut buf).unwrap();
                if n == 0 {
                    break;
                }
                raw.extend_from_slice(&buf[..n]);
                let text = String::from_utf8_lossy(&raw).to_string();
                if let Some(idx) = text.find("\r\n\r\n") {
                    let len = text[..idx]
                        .lines()
                        .find_map(|l| {
                            l.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .map(|v| v.trim().parse::<usize>().unwrap_or(0))
                        })
                        .unwrap_or(0);
                    if raw.len() >= idx + 4 + len {
                        break;
                    }
                }
            }
            let req = String::from_utf8_lossy(&raw).to_string();
            let body = r#"{"ok":true}"#;
            let _ = stream.write_all(
                format!(
                    "HTTP/1.1 201 Created\r\nContent-Type: application/json\r\nContent-Length: {}\r\nX-Echo: 1\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                )
                .as_bytes(),
            );
            req
        });

        let mut s = spec("POST", &format!("http://{addr}/hello"));
        s.params = vec![KeyValue::new("q", "1")];
        s.body_kind = "raw".into();
        s.body = r#"{"a":1}"#.into();
        s.options.timeout_ms = 5_000;

        let out = send(&s).unwrap();
        let seen = server.join().unwrap();

        assert!(seen.starts_with("POST /hello?q=1 HTTP/1.1"), "{seen}");
        assert!(seen.to_ascii_lowercase().contains("content-type: application/json"));
        assert!(seen.contains(r#"{"a":1}"#));
        assert_eq!(out.status, 201);
        assert_eq!(out.status_text, "Created");
        assert_eq!(out.body, r#"{"ok":true}"#);
        assert!(!out.is_base64);
        assert_eq!(out.body_bytes, 11);
        assert_eq!(header_value(&out.headers, "x-echo").unwrap(), "1");
    }

    #[test]
    fn send_reports_status_without_error() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf);
            let body = "nope";
            let _ = stream.write_all(
                format!(
                    "HTTP/1.1 404 Not Found\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                )
                .as_bytes(),
            );
        });

        let out = send(&spec("GET", &format!("http://{addr}/missing"))).unwrap();
        assert_eq!(out.status, 404);
        assert_eq!(out.body, "nope");
    }

    #[test]
    fn send_marks_binary_body_as_base64() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf);
            let _ = stream.write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: 3\r\nConnection: close\r\n\r\n\xff\x00\x01",
            );
        });

        let out = send(&spec("GET", &format!("http://{addr}/img"))).unwrap();
        assert!(out.is_base64);
        assert_eq!(out.body, B64.encode([0xffu8, 0x00, 0x01]));
    }
}
