use serde::Serialize;
use serde_json::Value;

use crate::{IktError, Result};

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JsonStats {
    pub bytes_in: usize,
    pub bytes_out: usize,
    pub nodes: usize,
    pub depth: usize,
    pub objects: usize,
    pub arrays: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JsonError {
    pub message: String,
    pub line: usize,
    pub column: usize,
    /// 错误行附近的一小段原文，便于界面直接高亮
    pub snippet: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JsonOutcome {
    pub ok: bool,
    pub output: String,
    pub error: Option<JsonError>,
    pub stats: JsonStats,
    /// 耗时微秒，用来在状态栏展示真实性能
    pub elapsed_us: u128,
}

/// 格式化 / 压缩 / 校验 JSON。
pub fn format(input: &str, indent: usize, sort_keys: bool, minify: bool) -> JsonOutcome {
    let started = std::time::Instant::now();
    let bytes_in = input.len();

    if input.trim().is_empty() {
        return fail(
            "输入为空".to_string(),
            1,
            1,
            String::new(),
            bytes_in,
            started.elapsed().as_micros(),
        );
    }

    let mut value: Value = match serde_json::from_str(input) {
        Ok(v) => v,
        Err(e) => {
            let line = e.line();
            let column = e.column();
            let snippet = line_snippet(input, line);
            return fail(
                explain(&e),
                line,
                column,
                snippet,
                bytes_in,
                started.elapsed().as_micros(),
            );
        }
    };

    if sort_keys {
        sort_recursive(&mut value);
    }

    let (nodes, depth, objects, arrays) = walk(&value);

    let output = if minify {
        serde_json::to_string(&value).unwrap_or_default()
    } else {
        let width = indent.clamp(1, 8);
        let pad = vec![b' '; width];
        let formatter = serde_json::ser::PrettyFormatter::with_indent(&pad);
        let mut buf: Vec<u8> = Vec::with_capacity(bytes_in);
        let mut ser = serde_json::Serializer::with_formatter(&mut buf, formatter);
        match value.serialize(&mut ser) {
            Ok(()) => String::from_utf8(buf).unwrap_or_default(),
            Err(e) => {
                return fail(
                    e.to_string(),
                    1,
                    1,
                    String::new(),
                    bytes_in,
                    started.elapsed().as_micros(),
                )
            }
        }
    };

    JsonOutcome {
        ok: true,
        stats: JsonStats {
            bytes_in,
            bytes_out: output.len(),
            nodes,
            depth,
            objects,
            arrays,
        },
        output,
        error: None,
        elapsed_us: started.elapsed().as_micros(),
    }
}

/// 只做校验，返回是否合法 + 结构统计，不产出文本（大文件场景更省内存）。
pub fn validate(input: &str) -> Result<JsonStats> {
    let value: Value = serde_json::from_str(input)
        .map_err(|e| IktError::Invalid(format!("第 {} 行第 {} 列：{}", e.line(), e.column(), explain(&e))))?;
    let (nodes, depth, objects, arrays) = walk(&value);
    Ok(JsonStats {
        bytes_in: input.len(),
        bytes_out: 0,
        nodes,
        depth,
        objects,
        arrays,
    })
}

fn fail(msg: String, line: usize, column: usize, snippet: String, bytes_in: usize, elapsed: u128) -> JsonOutcome {
    JsonOutcome {
        ok: false,
        output: String::new(),
        error: Some(JsonError { message: msg, line, column, snippet }),
        stats: JsonStats { bytes_in, ..Default::default() },
        elapsed_us: elapsed,
    }
}

/// serde_json 的报错信息对开发者够用，这里补一点中文引导。
fn explain(e: &serde_json::Error) -> String {
    let raw = e.to_string();
    if raw.contains("EOF while parsing") {
        format!("{raw}（JSON 提前结束，检查括号/引号是否闭合）")
    } else if raw.contains("expected value") {
        format!("{raw}（这里应该是一个值，可能多了逗号或缺少引号）")
    } else if raw.contains("trailing characters") {
        format!("{raw}（主体之后还有内容，通常是多打了一个 }} 或 ,）")
    } else if raw.contains("key must be a string") {
        format!("{raw}（对象的键必须用双引号包裹）")
    } else {
        raw
    }
}

fn line_snippet(input: &str, line: usize) -> String {
    input
        .lines()
        .nth(line.saturating_sub(1))
        .unwrap_or("")
        .chars()
        .take(200)
        .collect()
}

fn sort_recursive(v: &mut Value) {
    match v {
        Value::Object(map) => {
            let entries: Vec<(String, Value)> = std::mem::take(map).into_iter().collect();
            let mut entries = entries;
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            for (k, mut val) in entries {
                sort_recursive(&mut val);
                map.insert(k, val);
            }
        }
        Value::Array(arr) => {
            for val in arr.iter_mut() {
                sort_recursive(val);
            }
        }
        _ => {}
    }
}

type Shape = (usize, usize, usize, usize);

/// 返回 (节点数, 深度, 对象数, 数组数)
fn walk(v: &Value) -> Shape {
    fn rec(v: &Value, depth: usize, acc: &mut Shape) {
        acc.0 += 1;
        if depth > acc.1 {
            acc.1 = depth;
        }
        match v {
            Value::Object(map) => {
                acc.2 += 1;
                for (_, val) in map {
                    rec(val, depth + 1, acc);
                }
            }
            Value::Array(arr) => {
                acc.3 += 1;
                for val in arr {
                    rec(val, depth + 1, acc);
                }
            }
            _ => {}
        }
    }
    let mut acc: Shape = (0, 0, 0, 0);
    rec(v, 1, &mut acc);
    acc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_with_custom_indent() {
        let out = format(r#"{"b":1,"a":{"x":[1,2]}}"#, 4, false, false);
        assert!(out.ok);
        assert!(out.output.contains("    \"b\": 1"));
        assert_eq!(out.stats.objects, 2);
        assert_eq!(out.stats.arrays, 1);
        assert_eq!(out.stats.depth, 4);
    }

    #[test]
    fn sorts_keys_recursively() {
        let out = format(r#"{"b":1,"a":{"z":1,"y":2}}"#, 2, true, false);
        let a = out.output.find("\"a\"").unwrap();
        let b = out.output.find("\"b\"").unwrap();
        assert!(a < b);
        let y = out.output.find("\"y\"").unwrap();
        let z = out.output.find("\"z\"").unwrap();
        assert!(y < z);
    }

    #[test]
    fn minify_strips_whitespace() {
        let out = format("{\n  \"a\": [1, 2]\n}", 2, false, true);
        assert_eq!(out.output, r#"{"a":[1,2]}"#);
    }

    #[test]
    fn reports_error_position() {
        let out = format("{\n  \"a\": 1,\n  \"b\":\n}", 2, false, false);
        assert!(!out.ok);
        let err = out.error.unwrap();
        assert_eq!(err.line, 4);
        assert!(err.message.len() > 0);
    }

    #[test]
    fn empty_input_is_rejected() {
        let out = format("   ", 2, false, false);
        assert!(!out.ok);
    }

    /// 前后端契约：字段必须是 camelCase，改名必须同步改 src/lib/api.ts
    #[test]
    fn serializes_camel_case_contract() {
        let out = format("{\"a\":1}", 2, false, false);
        let v = serde_json::to_value(&out).unwrap();
        for key in ["ok", "output", "stats", "elapsedUs"] {
            assert!(v.get(key).is_some(), "缺少字段 {key}");
        }
        let stats = v.get("stats").unwrap();
        for key in ["bytesIn", "bytesOut", "nodes", "depth", "objects", "arrays"] {
            assert!(stats.get(key).is_some(), "stats 缺少字段 {key}");
        }
        assert!(v.get("error").unwrap().is_null(), "成功时 error 必须是 null");
    }
}
