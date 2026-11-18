//! IPC 薄封装层：只做参数校验和错误类型转换，业务逻辑全在 ikt-core。
//! 这样同一套逻辑将来可以直接给 CLI 用，不用抄第二遍。

use ikt_core::{crypto, encoding, http, json, jwt, net, random, regexp, rsa, timeutil, xml};

#[tauri::command(rename_all = "camelCase")]
pub fn json_format(input: String, indent: usize, sort_keys: bool, minify: bool) -> json::JsonOutcome {
    json::format(&input, indent, sort_keys, minify)
}

#[tauri::command(rename_all = "camelCase")]
pub fn digest(algorithm: String, input: String, key: Option<String>) -> Result<crypto::HashOutcome, String> {
    crypto::digest(&algorithm, &input, key.as_deref()).map_err(|e| e.to_string())
}

#[tauri::command(rename_all = "camelCase")]
pub fn aes_encrypt(spec: crypto::AesSpec, plaintext: String) -> Result<crypto::AesOutcome, String> {
    crypto::aes_encrypt(&spec, &plaintext).map_err(|e| e.to_string())
}

#[tauri::command(rename_all = "camelCase")]
pub fn aes_decrypt(spec: crypto::AesSpec, payload: String) -> Result<crypto::AesOutcome, String> {
    crypto::aes_decrypt(&spec, &payload).map_err(|e| e.to_string())
}

#[tauri::command(rename_all = "camelCase")]
pub fn base64_encode(input: String, url_safe: bool, no_pad: bool) -> String {
    encoding::base64_encode(&input, url_safe, no_pad)
}

#[tauri::command(rename_all = "camelCase")]
pub fn base64_decode(input: String) -> Result<String, String> {
    encoding::base64_decode(&input).map_err(|e| e.to_string())
}

#[tauri::command(rename_all = "camelCase")]
pub fn url_encode(spec: encoding::UrlSpec, input: String) -> encoding::UrlOutcome {
    encoding::url_encode(&input, &spec)
}

#[tauri::command(rename_all = "camelCase")]
pub fn url_decode(
    spec: encoding::UrlSpec,
    input: String,
) -> Result<encoding::UrlOutcome, String> {
    encoding::url_decode(&input, &spec).map_err(|e| e.to_string())
}

#[tauri::command(rename_all = "camelCase")]
pub fn ascii_encode(spec: encoding::AsciiSpec, input: String) -> encoding::AsciiOutcome {
    encoding::text_to_ascii(&input, &spec)
}

#[tauri::command(rename_all = "camelCase")]
pub fn ascii_decode(input: String) -> Result<encoding::AsciiOutcome, String> {
    encoding::ascii_to_text(&input).map_err(|e| e.to_string())
}

#[tauri::command(rename_all = "camelCase")]
pub fn regex_test(
    spec: regexp::RegexSpec,
    pattern: String,
    text: String,
) -> Result<regexp::RegexOutcome, String> {
    regexp::test(&spec, &pattern, &text).map_err(|e| e.to_string())
}

#[tauri::command(rename_all = "camelCase")]
pub fn regex_replace(
    spec: regexp::RegexSpec,
    pattern: String,
    text: String,
    replacement: String,
) -> Result<regexp::ReplaceOutcome, String> {
    regexp::replace(&spec, &pattern, &text, &replacement).map_err(|e| e.to_string())
}

#[tauri::command(rename_all = "camelCase")]
pub fn xml_json_to_xml(spec: xml::XmlSpec, input: String) -> Result<xml::XmlOutcome, String> {
    xml::json_to_xml(&spec, &input).map_err(|e| e.to_string())
}

#[tauri::command(rename_all = "camelCase")]
pub fn xml_xml_to_json(spec: xml::XmlSpec, input: String) -> Result<xml::XmlOutcome, String> {
    xml::xml_to_json(&spec, &input).map_err(|e| e.to_string())
}

#[tauri::command(rename_all = "camelCase")]
pub fn xml_format(spec: xml::XmlSpec, input: String) -> Result<xml::XmlOutcome, String> {
    xml::format(&spec, &input).map_err(|e| e.to_string())
}

#[tauri::command(rename_all = "camelCase")]
pub fn xml_minify(spec: xml::XmlSpec, input: String) -> Result<xml::XmlOutcome, String> {
    xml::minify(&spec, &input).map_err(|e| e.to_string())
}

#[tauri::command(rename_all = "camelCase")]
pub fn jwt_decode(token: String) -> Result<jwt::JwtParts, String> {
    jwt::decode(&token).map_err(|e| e.to_string())
}

// RSA 的三件事都丢到阻塞线程池：4096 位密钥生成可能要几秒，不能占着 UI 线程。

#[tauri::command(rename_all = "camelCase")]
pub async fn rsa_generate_keypair(bits: usize) -> Result<rsa::RsaKeyPair, String> {
    tauri::async_runtime::spawn_blocking(move || rsa::generate_keypair(bits))
        .await
        .map_err(|e| format!("密钥生成被中断：{e}"))?
        .map_err(|e| e.to_string())
}

#[tauri::command(rename_all = "camelCase")]
pub async fn rsa_encrypt(
    spec: rsa::RsaSpec,
    public_key: String,
    plaintext: String,
) -> Result<rsa::RsaOutcome, String> {
    tauri::async_runtime::spawn_blocking(move || rsa::rsa_encrypt(&spec, &public_key, &plaintext))
        .await
        .map_err(|e| format!("加密线程异常：{e}"))?
        .map_err(|e| e.to_string())
}

#[tauri::command(rename_all = "camelCase")]
pub async fn rsa_decrypt(
    spec: rsa::RsaSpec,
    private_key: String,
    payload: String,
) -> Result<rsa::RsaOutcome, String> {
    tauri::async_runtime::spawn_blocking(move || rsa::rsa_decrypt(&spec, &private_key, &payload))
        .await
        .map_err(|e| format!("解密线程异常：{e}"))?
        .map_err(|e| e.to_string())
}

#[tauri::command(rename_all = "camelCase")]
pub fn rsa_inspect(key: String) -> Result<rsa::RsaKeyInfo, String> {
    rsa::inspect(&key).map_err(|e| e.to_string())
}

// 归属地库直接嵌进二进制：11 MB，换来离线可用、不依赖安装路径。

static IP2REGION_V4: &[u8] = include_bytes!("../ip2region_v4.xdb");

#[tauri::command(rename_all = "camelCase")]
pub async fn net_ping(spec: net::PingSpec) -> Result<net::PingOutcome, String> {
    tauri::async_runtime::spawn_blocking(move || net::tcp_ping(&spec))
        .await
        .map_err(|e| format!("探测线程异常：{e}"))?
        .map_err(|e| e.to_string())
}

#[tauri::command(rename_all = "camelCase")]
pub async fn net_probe(spec: net::ProbeSpec) -> Result<net::ProbeOutcome, String> {
    tauri::async_runtime::spawn_blocking(move || net::tcp_probe(&spec))
        .await
        .map_err(|e| format!("探测线程异常：{e}"))?
        .map_err(|e| e.to_string())
}

#[tauri::command(rename_all = "camelCase")]
pub async fn net_dns(host: String) -> Result<net::DnsOutcome, String> {
    tauri::async_runtime::spawn_blocking(move || net::dns_lookup(&host))
        .await
        .map_err(|e| format!("解析线程异常：{e}"))?
        .map_err(|e| e.to_string())
}

#[tauri::command(rename_all = "camelCase")]
pub async fn net_geo(target: String) -> Result<net::GeoOutcome, String> {
    tauri::async_runtime::spawn_blocking(move || net::geo_lookup(&target, Some(IP2REGION_V4), None))
        .await
        .map_err(|e| format!("查询线程异常：{e}"))?
        .map_err(|e| e.to_string())
}

#[tauri::command(rename_all = "camelCase")]
pub fn random_generate(spec: random::RandomSpec) -> Result<random::RandomOutcome, String> {
    random::generate(&spec).map_err(|e| e.to_string())
}

#[tauri::command(rename_all = "camelCase")]
pub fn time_convert(spec: timeutil::TimeSpec, input: String) -> Result<timeutil::TimeOutcome, String> {
    timeutil::convert(&spec, &input).map_err(|e| e.to_string())
}

#[tauri::command(rename_all = "camelCase")]
pub fn time_now(spec: timeutil::TimeSpec) -> Result<timeutil::TimeOutcome, String> {
    timeutil::now(&spec).map_err(|e| e.to_string())
}

// HTTP 客户端是阻塞的（ureq），发送一定要丢到后台线程，否则界面会僵住。

#[tauri::command(rename_all = "camelCase")]
pub async fn http_build(spec: http::HttpSpec) -> Result<http::BuiltRequest, String> {
    http::build(&spec).map_err(|e| e.to_string())
}

#[tauri::command(rename_all = "camelCase")]
pub async fn http_send(spec: http::HttpSpec) -> Result<http::HttpOutcome, String> {
    tauri::async_runtime::spawn_blocking(move || http::send(&spec))
        .await
        .map_err(|e| format!("请求线程异常：{e}"))?
        .map_err(|e| e.to_string())
}
