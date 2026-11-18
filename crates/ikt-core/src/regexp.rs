//! 正则测试与替换。
//!
//! 引擎用 Rust 的 `regex`（线性时间、无回溯、不支持反向引用与环视），
//! 所以这里不做「超时」防护：任何输入都不会炸 CPU，只会因为表达式本身不合法而报错。

use regex::{Regex, RegexBuilder};
use serde::{Deserialize, Serialize};

use crate::{IktError, Result};

/// 命中上限：再多也只是把 UI 拖慢，超出部分只计数不返回。
const MAX_MATCHES: usize = 2000;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RegexSpec {
    /// `i` 忽略大小写
    pub ignore_case: bool,
    /// `m` `^` `$` 匹配每行首尾
    pub multi_line: bool,
    /// `s` `.` 也匹配换行
    pub dot_all: bool,
    /// `x` 忽略表达式里的空白（便于写长表达式）
    pub extended: bool,
    /// `u` Unicode 模式；关掉后 `\w` `\d` `\b` 退回 ASCII 语义
    pub unicode: bool,
}

impl Default for RegexSpec {
    fn default() -> Self {
        Self {
            ignore_case: false,
            multi_line: false,
            dot_all: false,
            extended: false,
            unicode: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegexGroup {
    /// 捕获组序号（从 1 开始，0 是整体匹配）
    pub index: usize,
    /// 具名分组的名字，如 `(?P<year>...)` 里的 `year`
    pub name: Option<String>,
    /// 没参与匹配的组为 `None`（如 `(a)|(b)` 里没走到的那个分支）
    pub text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegexMatch {
    /// 从 1 开始，跟界面上的 `#1` 对齐
    pub index: usize,
    /// 字节偏移（含头不含尾），用于和 Rust / 其他语言对齐
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub column: usize,
    pub text: String,
    pub groups: Vec<RegexGroup>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegexOutcome {
    pub matches: Vec<RegexMatch>,
    /// 命中总数（可能大于 `matches.len()`，见 `truncated`）
    pub count: usize,
    pub truncated: bool,
    /// 表达式里的全部具名分组，按出现顺序
    pub group_names: Vec<String>,
    pub elapsed_us: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplaceOutcome {
    pub output: String,
    pub output_bytes: usize,
    pub replacements: usize,
    pub changed: bool,
    pub elapsed_us: u128,
}

fn pretty_error(e: &regex::Error) -> String {
    let raw = e.to_string();
    let detail = raw
        .lines()
        .find_map(|l| l.trim().strip_prefix("error: "))
        .unwrap_or(raw.trim());
    format!("正则语法错误：{detail}")
}

fn build(pattern: &str, spec: &RegexSpec) -> Result<Regex> {
    if pattern.trim().is_empty() {
        return Err(IktError::Invalid("正则表达式为空".into()));
    }
    RegexBuilder::new(pattern)
        .case_insensitive(spec.ignore_case)
        .multi_line(spec.multi_line)
        .dot_matches_new_line(spec.dot_all)
        .ignore_whitespace(spec.extended)
        .unicode(spec.unicode)
        .build()
        .map_err(|e| IktError::Invalid(pretty_error(&e)))
}

fn line_starts(text: &str) -> Vec<usize> {
    let mut starts = vec![0usize];
    for (i, b) in text.bytes().enumerate() {
        if b == b'\n' {
            starts.push(i + 1);
        }
    }
    starts
}

/// 字节偏移 → (行号, 列号)，都从 1 开始，列号按字符算（中文占 1 列）。
fn locate(starts: &[usize], text: &str, offset: usize) -> (usize, usize) {
    let idx = match starts.binary_search(&offset) {
        Ok(i) => i,
        Err(i) => i.saturating_sub(1),
    };
    let column = text[starts[idx]..offset].chars().count() + 1;
    (idx + 1, column)
}

/// 跑一遍表达式，返回每一处命中的位置、文本与捕获组。
pub fn test(spec: &RegexSpec, pattern: &str, text: &str) -> Result<RegexOutcome> {
    let started = std::time::Instant::now();
    let re = build(pattern, spec)?;
    let starts = line_starts(text);
    let names: Vec<Option<String>> = re
        .capture_names()
        .map(|n| n.map(|s| s.to_string()))
        .collect();

    let mut matches: Vec<RegexMatch> = Vec::new();
    let mut count = 0usize;
    for caps in re.captures_iter(text) {
        count += 1;
        if matches.len() >= MAX_MATCHES {
            continue;
        }
        let whole = caps.get(0).expect("第 0 组一定存在");
        let (line, column) = locate(&starts, text, whole.start());
        let mut groups = Vec::new();
        for gi in 1..caps.len() {
            groups.push(RegexGroup {
                index: gi,
                name: names.get(gi).cloned().flatten(),
                text: caps.get(gi).map(|g| g.as_str().to_string()),
            });
        }
        matches.push(RegexMatch {
            index: matches.len() + 1,
            start: whole.start(),
            end: whole.end(),
            line,
            column,
            text: whole.as_str().to_string(),
            groups,
        });
    }

    Ok(RegexOutcome {
        truncated: count > matches.len(),
        group_names: names.into_iter().flatten().collect(),
        count,
        matches,
        elapsed_us: started.elapsed().as_micros(),
    })
}

/// 替换全部命中。替换串支持 `$0` / `$1` / `${1}` / `$name` / `${name}`，`$$` 表示字面 `$`。
pub fn replace(
    spec: &RegexSpec,
    pattern: &str,
    text: &str,
    replacement: &str,
) -> Result<ReplaceOutcome> {
    let started = std::time::Instant::now();
    let re = build(pattern, spec)?;
    let replacements = re.find_iter(text).count();
    let output = re.replace_all(text, replacement).into_owned();
    let changed = replacements > 0 && output != text;

    Ok(ReplaceOutcome {
        output_bytes: output.len(),
        output,
        replacements,
        changed,
        elapsed_us: started.elapsed().as_micros(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec() -> RegexSpec {
        RegexSpec::default()
    }

    #[test]
    fn finds_all_matches_with_positions() {
        let text = "abc\n123 def456";
        let out = test(&spec(), r"\d+", text).unwrap();
        assert_eq!(out.count, 2);
        assert_eq!(out.matches[0].text, "123");
        assert_eq!((out.matches[0].start, out.matches[0].end), (4, 7));
        assert_eq!((out.matches[0].line, out.matches[0].column), (2, 1));
        assert_eq!(out.matches[1].text, "456");
        assert_eq!((out.matches[1].line, out.matches[1].column), (2, 8));
        assert_eq!(out.matches[1].index, 2);
        assert!(!out.truncated);
    }

    #[test]
    fn column_counts_characters_not_bytes() {
        // 中文每字 3 字节，列号要按字符算
        let out = test(&spec(), "b", "中文字b").unwrap();
        assert_eq!((out.matches[0].line, out.matches[0].column), (1, 4));
        assert_eq!(out.matches[0].start, 9);
    }

    #[test]
    fn captures_named_and_unnamed() {
        let out = test(&spec(), r"(?P<year>\d{4})-(\d{2})", "2026-11").unwrap();
        assert_eq!(out.group_names, vec!["year".to_string()]);
        let g = &out.matches[0].groups;
        assert_eq!(g.len(), 2);
        assert_eq!(g[0].name.as_deref(), Some("year"));
        assert_eq!(g[0].text.as_deref(), Some("2026"));
        assert_eq!(g[1].name, None);
        assert_eq!(g[1].text.as_deref(), Some("11"));
    }

    #[test]
    fn unmatched_alternative_group_is_none() {
        let out = test(&spec(), r"(a)|(b)", "b").unwrap();
        assert_eq!(out.matches[0].groups[0].text, None);
        assert_eq!(out.matches[0].groups[1].text.as_deref(), Some("b"));
    }

    #[test]
    fn flags_change_matching() {
        let text = "Hello\nworld";
        assert_eq!(test(&spec(), "hello", text).unwrap().count, 0);
        let i = RegexSpec {
            ignore_case: true,
            ..Default::default()
        };
        assert_eq!(test(&i, "hello", text).unwrap().count, 1);

        // 默认 ^ 只匹配全文开头
        assert_eq!(test(&spec(), "^world", text).unwrap().count, 0);
        let m = RegexSpec {
            multi_line: true,
            ..Default::default()
        };
        assert_eq!(test(&m, "^world", text).unwrap().count, 1);

        // 默认 . 不跨行
        assert_eq!(test(&spec(), "Hello.world", text).unwrap().count, 0);
        let s = RegexSpec {
            dot_all: true,
            ..Default::default()
        };
        assert_eq!(test(&s, "Hello.world", text).unwrap().count, 1);

        // x 忽略表达式中的空白
        let x = RegexSpec {
            extended: true,
            ..Default::default()
        };
        assert_eq!(test(&x, r"\d {2} - \d {2}", "12-34").unwrap().count, 1);
    }

    #[test]
    fn unicode_flag_controls_word_class() {
        assert_eq!(test(&spec(), r"\w+", "中文").unwrap().count, 1);
        let ascii = RegexSpec {
            unicode: false,
            ..Default::default()
        };
        assert_eq!(test(&ascii, r"\w+", "中文").unwrap().count, 0);
    }

    #[test]
    fn zero_length_matches_advance() {
        let out = test(&spec(), r"\b", "ab cd").unwrap();
        assert_eq!(out.count, 4);
        assert_eq!(out.matches[0].text, "");
    }

    #[test]
    fn caps_match_count() {
        let text = "a".repeat(MAX_MATCHES + 5);
        let out = test(&spec(), "a", &text).unwrap();
        assert_eq!(out.count, MAX_MATCHES + 5);
        assert_eq!(out.matches.len(), MAX_MATCHES);
        assert!(out.truncated);
    }

    #[test]
    fn replace_with_group_refs() {
        let out = replace(&spec(), r"(\w+)@(\w+)", "a@b c@d", "$2:$1").unwrap();
        assert_eq!(out.output, "b:a d:c");
        assert_eq!(out.replacements, 2);
        assert!(out.changed);
    }

    #[test]
    fn replace_supports_named_and_literal_dollar() {
        let out = replace(&spec(), r"(?P<n>\d+)", "x 42 y", "<${n}>$$").unwrap();
        assert_eq!(out.output, "x <42>$ y");
        assert_eq!(out.replacements, 1);
    }

    #[test]
    fn replace_reports_no_change() {
        let out = replace(&spec(), r"\d+", "abc", "x").unwrap();
        assert_eq!(out.output, "abc");
        assert_eq!(out.replacements, 0);
        assert!(!out.changed);
    }

    #[test]
    fn invalid_pattern_reports_single_line_error() {
        let err = test(&spec(), "[a-", "x").unwrap_err().to_string();
        assert!(err.starts_with("正则语法错误："), "{err}");
        assert!(!err.contains('\n'), "错误信息不该带换行：{err}");

        let err = test(&spec(), "  ", "x").unwrap_err().to_string();
        assert!(err.contains("为空"), "{err}");
    }

    #[test]
    fn contract_is_camel_case() {
        let spec = serde_json::to_value(RegexSpec::default()).unwrap();
        for key in ["ignoreCase", "multiLine", "dotAll", "extended", "unicode"] {
            assert!(spec.get(key).is_some(), "缺字段 {key}");
        }

        let out = serde_json::to_value(test(&RegexSpec::default(), r"(?P<a>x)", "x").unwrap()).unwrap();
        for key in ["matches", "count", "truncated", "groupNames", "elapsedUs"] {
            assert!(out.get(key).is_some(), "缺字段 {key}");
        }
        let m = &out["matches"][0];
        for key in ["index", "start", "end", "line", "column", "text", "groups"] {
            assert!(m.get(key).is_some(), "缺字段 {key}");
        }
        assert_eq!(m["groups"][0]["name"], "a");

        let r = serde_json::to_value(replace(&RegexSpec::default(), "a", "a", "b").unwrap()).unwrap();
        for key in ["output", "outputBytes", "replacements", "changed", "elapsedUs"] {
            assert!(r.get(key).is_some(), "缺字段 {key}");
        }
    }
}
