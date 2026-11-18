//! 时间戳 ⇄ 日期互转。
//!
//! 只有一个入口：输入既可以是时间戳（自动识别秒 / 毫秒 / 微秒 / 纳秒），
//! 也可以是日期字符串（RFC 3339、ISO 8601、常见 `YYYY-MM-DD HH:MM:SS` 写法）。
//! 输出统一包含秒 / 毫秒 / 微秒 / 纳秒、UTC 与目标时区的两种写法、周几、年内第几天、第几周和相对时间。

use std::time::Instant;

use chrono::{DateTime, Datelike, FixedOffset, NaiveDate, NaiveDateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};

use crate::{IktError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimeSpec {
    /// 时区偏移，写法 `+08:00` / `-05:00` / `Z` / `UTC`。
    pub tz: String,
    /// `auto` / `s` / `ms` / `us` / `ns`，仅在输入是纯数字时生效。
    pub unit: String,
}

impl Default for TimeSpec {
    fn default() -> Self {
        Self {
            tz: "+08:00".into(),
            unit: "auto".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimeOutcome {
    /// `toDate`（数字 → 日期）或 `toTimestamp`（日期 → 数字）。
    pub direction: String,
    /// 实际使用的单位。
    pub unit: String,
    pub seconds: i64,
    pub millis: i64,
    pub micros: i64,
    pub nanos: i64,
    pub utc_iso: String,
    pub utc_text: String,
    pub local_iso: String,
    pub local_text: String,
    pub tz_label: String,
    pub weekday: String,
    pub day_of_year: u32,
    pub week: u32,
    pub relative: String,
    pub now_ms: i64,
    pub elapsed_us: u64,
}

const NAIVE_FORMATS: [&str; 10] = [
    "%Y-%m-%d %H:%M:%S%.f",
    "%Y-%m-%dT%H:%M:%S%.f",
    "%Y-%m-%dT%H:%M",
    "%Y-%m-%d %H:%M",
    "%Y-%m-%d",
    "%Y/%m/%d %H:%M:%S%.f",
    "%Y/%m/%d %H:%M",
    "%Y/%m/%d",
    "%Y%m%d%H%M%S",
    "%Y%m%d",
];

/// 解析 `+08:00` / `-0530` / `Z` / `UTC` / `GMT+8` 这类偏移写法。
pub fn parse_offset(tz: &str) -> Result<FixedOffset> {
    let raw = tz.trim();
    if raw.is_empty() {
        return Err(IktError::Invalid("时区不能为空".into()));
    }
    let upper = raw.to_ascii_uppercase();
    if upper == "Z" || upper == "UTC" || upper == "GMT" || upper == "+00:00" || upper == "0" {
        return FixedOffset::east_opt(0).ok_or_else(|| IktError::Internal("时区构造失败".into()));
    }
    let body = upper
        .strip_prefix("UTC")
        .or_else(|| upper.strip_prefix("GMT"))
        .unwrap_or(&upper);
    let (sign, rest) = match body.chars().next() {
        Some('+') => (1, &body[1..]),
        Some('-') => (-1, &body[1..]),
        _ => (1, body),
    };
    let digits: String = rest.chars().filter(|c| c.is_ascii_digit()).collect();
    let (h, m) = match digits.len() {
        1 | 2 => (digits.parse::<i32>().unwrap_or(-1), 0),
        3 => (
            digits[..1].parse::<i32>().unwrap_or(-1),
            digits[1..].parse::<i32>().unwrap_or(-1),
        ),
        _ => (
            digits[..2].parse::<i32>().unwrap_or(-1),
            digits[2..4].parse::<i32>().unwrap_or(-1),
        ),
    };
    if !(0..=23).contains(&h) || !(0..=59).contains(&m) {
        return Err(IktError::Invalid(format!("时区写法不对：{tz}")));
    }
    FixedOffset::east_opt(sign * (h * 3600 + m * 60))
        .ok_or_else(|| IktError::Invalid(format!("时区写法不对：{tz}")))
}

fn detect_unit(digits: usize) -> &'static str {
    match digits {
        0..=11 => "s",
        12..=14 => "ms",
        15..=17 => "us",
        _ => "ns",
    }
}

/// 把纯数字输入换算成 (秒, 亚秒纳秒)。
fn numeric_to_parts(raw: &str, unit: &str) -> Result<(i64, u32)> {
    let (sign, body) = match raw.chars().next() {
        Some('-') => (-1i128, &raw[1..]),
        Some('+') => (1i128, &raw[1..]),
        _ => (1i128, raw),
    };
    let (int_part, frac_part) = match body.split_once('.') {
        Some((a, b)) => (a, b),
        None => (body, ""),
    };
    if int_part.is_empty() {
        return Err(IktError::Invalid("时间戳格式不对".into()));
    }
    let int_val: i128 = int_part
        .parse()
        .map_err(|_| IktError::Invalid("时间戳超出范围".into()))?;
    let scale: i128 = match unit {
        "s" => 1_000_000_000,
        "ms" => 1_000_000,
        "us" => 1_000,
        _ => 1,
    };
    let mut nanos = int_val
        .checked_mul(scale)
        .ok_or_else(|| IktError::Invalid("时间戳超出范围".into()))?;
    if !frac_part.is_empty() {
        let digits: usize = match unit {
            "s" => 9,
            "ms" => 6,
            "us" => 3,
            _ => 0,
        };
        if digits > 0 {
            let mut frac = String::from(frac_part);
            frac.truncate(digits);
            while frac.len() < digits {
                frac.push('0');
            }
            let frac_val: i128 = frac
                .parse()
                .map_err(|_| IktError::Invalid("时间戳格式不对".into()))?;
            nanos += frac_val;
        }
    }
    nanos *= sign;
    let secs = nanos.div_euclid(1_000_000_000);
    let sub = nanos.rem_euclid(1_000_000_000);
    let secs: i64 = secs
        .try_into()
        .map_err(|_| IktError::Invalid("时间戳超出范围".into()))?;
    Ok((secs, sub as u32))
}

fn parse_datetime(raw: &str, tz: FixedOffset) -> Result<DateTime<FixedOffset>> {
    if let Ok(dt) = DateTime::parse_from_rfc3339(raw) {
        return Ok(dt);
    }
    // 2026-11-17 14:24:00 +08:00 / 2026-11-17T14:24:00+0800
    for fmt in ["%Y-%m-%d %H:%M:%S%.f %:z", "%Y-%m-%d %H:%M:%S%.f%:z"] {
        if let Ok(dt) = DateTime::parse_from_str(raw, fmt) {
            return Ok(dt);
        }
    }
    // 末尾带 Z 但 chrono 的 rfc3339 不认（缺秒）的情况，先按无时区解析再补
    let trimmed = raw.trim_end_matches('Z').trim_end_matches('z').trim();
    for fmt in NAIVE_FORMATS {
        if let Ok(dt) = NaiveDateTime::parse_from_str(trimmed, fmt) {
            return tz
                .from_local_datetime(&dt)
                .single()
                .ok_or_else(|| IktError::Invalid("这个时间在该时区不存在".into()));
        }
        if let Ok(d) = NaiveDate::parse_from_str(trimmed, fmt) {
            let dt = d
                .and_hms_opt(0, 0, 0)
                .ok_or_else(|| IktError::Internal("时间构造失败".into()))?;
            return tz
                .from_local_datetime(&dt)
                .single()
                .ok_or_else(|| IktError::Invalid("这个时间在该时区不存在".into()));
        }
    }
    Err(IktError::Invalid(format!(
        "认不出这个时间：{raw}（可试 2026-11-17 14:24:00 或 2026-11-17T14:24:00+08:00）"
    )))
}

fn weekday_cn(dt: &DateTime<FixedOffset>) -> String {
    match dt.weekday().number_from_monday() {
        1 => "周一",
        2 => "周二",
        3 => "周三",
        4 => "周四",
        5 => "周五",
        6 => "周六",
        _ => "周日",
    }
    .to_string()
}

fn relative_cn(target: i64, now: i64) -> String {
    let diff = target - now;
    let abs = diff.unsigned_abs() as f64;
    let suffix = if diff >= 0 { "后" } else { "前" };
    if abs < 1.0 {
        return "刚刚".into();
    }
    if abs < 60.0 {
        return format!("{}{}", abs as u64, if diff >= 0 { " 秒后" } else { " 秒前" });
    }
    let minutes = abs / 60.0;
    if minutes < 60.0 {
        return format!("{} 分钟{suffix}", minutes as u64);
    }
    let hours = minutes / 60.0;
    if hours < 24.0 {
        return format!("{} 小时{suffix}", hours as u64);
    }
    let days = hours / 24.0;
    if days < 30.0 {
        return format!("{} 天{suffix}", days as u64);
    }
    let months = days / 30.0;
    if months < 12.0 {
        return format!("{} 个月{suffix}", months as u64);
    }
    format!("{:.1} 年{suffix}", months / 12.0)
}

fn build(dt: DateTime<FixedOffset>, tz: FixedOffset, direction: &str, unit: &str) -> TimeOutcome {
    let started = Instant::now();
    let utc = dt.with_timezone(&Utc);
    let local = dt.with_timezone(&tz);
    let tz_label = tz.to_string();
    let now_ms = Utc::now().timestamp_millis();
    let seconds = dt.timestamp();
    TimeOutcome {
        direction: direction.to_string(),
        unit: unit.to_string(),
        seconds,
        millis: seconds * 1000 + (dt.timestamp_subsec_millis() as i64),
        micros: seconds * 1_000_000 + (dt.timestamp_subsec_micros() as i64),
        nanos: seconds * 1_000_000_000 + (dt.timestamp_subsec_nanos() as i64),
        utc_iso: utc.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
        utc_text: utc.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
        local_iso: local.format("%Y-%m-%dT%H:%M:%S%.3f%:z").to_string(),
        local_text: local.format("%Y-%m-%d %H:%M:%S").to_string(),
        tz_label,
        weekday: weekday_cn(&local),
        day_of_year: local.ordinal(),
        week: local.iso_week().week(),
        relative: relative_cn(seconds, now_ms / 1000),
        now_ms,
        elapsed_us: started.elapsed().as_micros() as u64,
    }
}

pub fn convert(spec: &TimeSpec, input: &str) -> Result<TimeOutcome> {
    let raw = input.trim();
    if raw.is_empty() {
        return Err(IktError::Invalid("输入为空".into()));
    }
    let tz = parse_offset(&spec.tz)?;

    let body = raw.trim_start_matches(['-', '+']);
    let is_numeric = !body.is_empty()
        && body.chars().all(|c| c.is_ascii_digit() || c == '.')
        && body.chars().any(|c| c.is_ascii_digit())
        && body.matches('.').count() <= 1;

    if is_numeric {
        let digits = body.split('.').next().unwrap_or("").trim_start_matches('0').len();
        let unit = match spec.unit.trim() {
            "" | "auto" => detect_unit(if digits == 0 { 1 } else { digits }),
            other @ ("s" | "ms" | "us" | "ns") => other,
            other => {
                return Err(IktError::Invalid(format!(
                    "单位只能是 auto / s / ms / us / ns：{other}"
                )))
            }
        };
        let (secs, nanos) = numeric_to_parts(raw, unit)?;
        let utc = DateTime::from_timestamp(secs, nanos)
            .ok_or_else(|| IktError::Invalid("时间戳超出可表示范围".into()))?;
        let dt = utc.with_timezone(&tz);
        return Ok(build(dt, tz, "toDate", unit));
    }

    let dt = parse_datetime(raw, tz)?;
    Ok(build(dt, tz, "toTimestamp", "s"))
}

/// 取当前时间，供界面上的「现在」按钮使用。
pub fn now(spec: &TimeSpec) -> Result<TimeOutcome> {
    let tz = parse_offset(&spec.tz)?;
    Ok(build(Utc::now().with_timezone(&tz), tz, "toDate", "s"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(tz: &str, unit: &str) -> TimeSpec {
        TimeSpec {
            tz: tz.into(),
            unit: unit.into(),
        }
    }

    #[test]
    fn offset_parsing() {
        assert_eq!(parse_offset("+08:00").unwrap().local_minus_utc(), 28800);
        assert_eq!(parse_offset("Z").unwrap().local_minus_utc(), 0);
        assert_eq!(parse_offset("UTC").unwrap().local_minus_utc(), 0);
        assert_eq!(parse_offset("-0530").unwrap().local_minus_utc(), -19800);
        assert_eq!(parse_offset("+9").unwrap().local_minus_utc(), 32400);
        assert!(parse_offset("+99:00").is_err());
        assert!(parse_offset("").is_err());
    }

    #[test]
    fn seconds_to_date() {
        let out = convert(&spec("+08:00", "auto"), "1700000000").unwrap();
        assert_eq!(out.direction, "toDate");
        assert_eq!(out.unit, "s");
        assert_eq!(out.utc_iso, "2023-11-14T22:13:20.000Z");
        assert_eq!(out.local_iso, "2023-11-15T06:13:20.000+08:00");
        assert_eq!(out.weekday, "周三");
        assert_eq!(out.millis, 1_700_000_000_000);
        assert_eq!(out.micros, 1_700_000_000_000_000);
        assert_eq!(out.nanos, 1_700_000_000_000_000_000);
    }

    #[test]
    fn millis_auto_detected() {
        let out = convert(&spec("+08:00", "auto"), "1700000000000").unwrap();
        assert_eq!(out.unit, "ms");
        assert_eq!(out.seconds, 1_700_000_000);
    }

    #[test]
    fn explicit_unit_wins() {
        let out = convert(&spec("Z", "ms"), "1700000000").unwrap();
        assert_eq!(out.unit, "ms");
        assert_eq!(out.seconds, 1_700_000);
    }

    #[test]
    fn fractional_seconds() {
        let out = convert(&spec("Z", "s"), "1700000000.250").unwrap();
        assert_eq!(out.millis, 1_700_000_000_250);
    }

    #[test]
    fn negative_timestamp() {
        let out = convert(&spec("Z", "s"), "-1").unwrap();
        assert_eq!(out.utc_iso, "1969-12-31T23:59:59.000Z");
    }

    #[test]
    fn rfc3339_with_offset() {
        let out = convert(&spec("+00:00", "auto"), "2023-11-15T06:13:20+08:00").unwrap();
        assert_eq!(out.direction, "toTimestamp");
        assert_eq!(out.seconds, 1_700_000_000);
        // 输入自带 +08:00 优先于所选时区；所选时区是 +00:00，所以本地写法就是 UTC
        assert_eq!(out.local_text, "2023-11-14 22:13:20");
    }

    #[test]
    fn naive_datetime_uses_spec_tz() {
        let out = convert(&spec("+08:00", "auto"), "2023-11-15 06:13:20").unwrap();
        assert_eq!(out.seconds, 1_700_000_000);
        let utc = convert(&spec("Z", "auto"), "2023-11-15 06:13:20").unwrap();
        assert_eq!(utc.seconds, 1_700_028_800);
    }

    #[test]
    fn date_only() {
        let out = convert(&spec("+08:00", "auto"), "2026-11-17").unwrap();
        assert_eq!(out.local_text, "2026-11-17 00:00:00");
        assert_eq!(out.day_of_year, 321);
    }

    #[test]
    fn space_separated_offset() {
        let out = convert(&spec("Z", "auto"), "2026-11-17 14:24:00 +08:00").unwrap();
        assert_eq!(out.utc_text, "2026-11-17 06:24:00 UTC");
    }

    #[test]
    fn rejects_garbage() {
        assert!(convert(&spec("+08:00", "auto"), "hello").is_err());
        assert!(convert(&spec("+08:00", "auto"), "2026-13-45").is_err());
        assert!(convert(&spec("+08:00", "auto"), "").is_err());
        assert!(convert(&spec("+08:00", "auto"), "abc123").is_err());
    }

    #[test]
    fn relative_phrases() {
        let now = 1_700_000_000;
        assert_eq!(relative_cn(now, now), "刚刚");
        assert_eq!(relative_cn(now - 30, now), "30 秒前");
        assert_eq!(relative_cn(now + 90, now), "1 分钟后");
        assert_eq!(relative_cn(now - 7200, now), "2 小时前");
        assert_eq!(relative_cn(now - 86400 * 3, now), "3 天前");
        assert_eq!(relative_cn(now - 86400 * 45, now), "1 个月前");
        assert_eq!(relative_cn(now - 86400 * 800, now), "2.2 年前");
    }

    #[test]
    fn contract_is_camel_case() {
        let json = serde_json::to_string(&convert(&spec("+08:00", "auto"), "1700000000").unwrap())
            .unwrap();
        for key in [
            "\"utcIso\"",
            "\"utcText\"",
            "\"localIso\"",
            "\"localText\"",
            "\"tzLabel\"",
            "\"dayOfYear\"",
            "\"nowMs\"",
            "\"elapsedUs\"",
            "\"direction\"",
        ] {
            assert!(json.contains(key), "缺少 {key}：{json}");
        }
    }
}
