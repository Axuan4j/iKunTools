//! XML 工具：JSON ↔ XML 互转、格式化、压缩。
//!
//! 统一做法是先把 XML 解析成一棵轻量树，再按需要序列化回 XML 或转成 JSON，
//! 三种能力共用同一套结构，不会出现「格式化能过、转 JSON 报错」的分叉。
//!
//! 转换约定（与 fast-xml-parser 的默认行为一致）：
//! - 属性 → `@name`
//! - 元素文本 → `#text`（只有同时存在属性或子元素时才会出现）
//! - 重复的同名子元素 → 数组
//! - 数组转 XML 时子元素沿用父键名，保证 XML → JSON → XML 能原样往返

use std::time::Instant;

use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::{IktError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct XmlSpec {
    /// 缩进宽度；0 表示不换行不缩进
    pub indent: usize,
    /// 输出 `<?xml version="1.0" encoding="UTF-8"?>`
    pub declaration: bool,
    /// JSON → XML 时根元素的名字
    pub root: String,
    /// 属性前缀：JSON 里以该前缀开头的键会成为属性；留空则全部当子元素
    pub attr_prefix: String,
    /// 元素文本在 JSON 里使用的键名
    pub text_key: String,
    /// XML → JSON 时是否去掉文本两端空白
    pub trim: bool,
    /// XML → JSON 时把 `42` / `3.14` / `true` 识别成数字与布尔
    pub numbers: bool,
    /// 格式化与压缩时是否保留注释
    pub keep_comments: bool,
}

impl Default for XmlSpec {
    fn default() -> Self {
        Self {
            indent: 2,
            declaration: true,
            root: "root".into(),
            attr_prefix: "@".into(),
            text_key: "#text".into(),
            trim: true,
            numbers: true,
            keep_comments: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct XmlOutcome {
    pub output: String,
    pub output_bytes: usize,
    /// 根元素名
    pub root: Option<String>,
    pub elements: usize,
    pub attributes: usize,
    /// 最大嵌套深度，根元素算 1
    pub depth: usize,
    pub comments: usize,
    /// JSON → XML 时被自动修正为合法 XML 名的键数量
    pub renamed: usize,
    pub elapsed_us: u128,
}

// ---------------------------------------------------------------- 文档树

#[derive(Debug, Clone)]
struct Element {
    name: String,
    attrs: Vec<(String, String)>,
    children: Vec<Node>,
}

#[derive(Debug, Clone)]
enum Node {
    Element(Element),
    Text(String),
    CData(String),
    Comment(String),
    /// 处理指令，如 `<?xml-stylesheet href="a"?>` 里的 `xml-stylesheet href="a"`
    Pi(String),
}

#[derive(Debug, Default)]
struct Doc {
    /// (version, encoding)
    decl: Option<(String, String)>,
    doctype: Option<String>,
    nodes: Vec<Node>,
}

// ---------------------------------------------------------------- 解析

fn position_err(src: &str, pos: usize, detail: impl Into<String>) -> IktError {
    let mut pos = pos.min(src.len());
    while pos > 0 && !src.is_char_boundary(pos) {
        pos -= 1;
    }
    let before = &src[..pos];
    let line = before.matches('\n').count() + 1;
    let column = before.rsplit('\n').next().map(|s| s.chars().count()).unwrap_or(0) + 1;
    IktError::Invalid(format!(
        "XML 语法错误（第 {line} 行第 {column} 列）：{}",
        detail.into()
    ))
}

fn read_name(e: &BytesStart) -> String {
    String::from_utf8_lossy(e.name().as_ref()).to_string()
}

fn read_attrs(e: &BytesStart, src: &str, pos: usize) -> Result<Vec<(String, String)>> {
    let mut out = Vec::new();
    for attr in e.attributes() {
        let attr = match attr {
            Ok(a) => a,
            Err(err) => return Err(position_err(src, pos, format!("属性不合法：{err}"))),
        };
        let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
        let value = match attr.unescape_value() {
            Ok(v) => v.to_string(),
            Err(err) => {
                return Err(position_err(src, pos, format!("属性 {key} 的值不合法：{err}")))
            }
        };
        out.push((key, value));
    }
    Ok(out)
}

fn parse(src: &str) -> Result<Doc> {
    let mut reader = Reader::from_str(src);
    let cfg = reader.config_mut();
    cfg.trim_text(false);
    cfg.check_end_names = true;

    let mut doc = Doc::default();
    let mut stack: Vec<Element> = Vec::new();

    loop {
        let pos = reader.buffer_position() as usize;
        let event = reader
            .read_event()
            .map_err(|e| position_err(src, pos, e.to_string().lines().next().unwrap_or("无法解析")))?;
        match event {
            Event::Start(e) => {
                stack.push(Element {
                    name: read_name(&e),
                    attrs: read_attrs(&e, src, pos)?,
                    children: Vec::new(),
                });
            }
            Event::Empty(e) => {
                let node = Node::Element(Element {
                    name: read_name(&e),
                    attrs: read_attrs(&e, src, pos)?,
                    children: Vec::new(),
                });
                match stack.last_mut() {
                    Some(parent) => parent.children.push(node),
                    None => doc.nodes.push(node),
                }
            }
            Event::End(e) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let el = match stack.pop() {
                    Some(el) => el,
                    None => return Err(position_err(src, pos, format!("多余的结束标签 </{name}>"))),
                };
                if el.name != name {
                    return Err(position_err(
                        src,
                        pos,
                        format!("结束标签 </{name}> 与开始标签 <{}> 不匹配", el.name),
                    ));
                }
                let node = Node::Element(el);
                match stack.last_mut() {
                    Some(parent) => parent.children.push(node),
                    None => doc.nodes.push(node),
                }
            }
            Event::Text(t) => {
                let text = t
                    .unescape()
                    .map_err(|e| position_err(src, pos, format!("文本里的实体不合法：{e}")))?
                    .to_string();
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(Node::Text(text));
                }
            }
            Event::CData(c) => {
                let text = String::from_utf8_lossy(c.as_ref()).to_string();
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(Node::CData(text));
                }
            }
            Event::Comment(c) => {
                let node = Node::Comment(String::from_utf8_lossy(c.as_ref()).to_string());
                match stack.last_mut() {
                    Some(parent) => parent.children.push(node),
                    None => doc.nodes.push(node),
                }
            }
            Event::PI(p) => {
                let node = Node::Pi(String::from_utf8_lossy(p.as_ref()).to_string());
                match stack.last_mut() {
                    Some(parent) => parent.children.push(node),
                    None => doc.nodes.push(node),
                }
            }
            Event::Decl(d) => {
                let version = d
                    .version()
                    .map(|v| String::from_utf8_lossy(&v).to_string())
                    .unwrap_or_else(|_| "1.0".into());
                let encoding = d
                    .encoding()
                    .and_then(|r| r.ok())
                    .map(|v| String::from_utf8_lossy(&v).to_string())
                    .unwrap_or_else(|| "UTF-8".into());
                doc.decl = Some((version, encoding));
            }
            Event::DocType(d) => {
                doc.doctype = Some(String::from_utf8_lossy(d.as_ref()).trim().to_string());
            }
            Event::Eof => break,
        }
    }

    if let Some(el) = stack.last() {
        return Err(IktError::Invalid(format!(
            "XML 语法错误：元素 <{}> 没有闭合",
            el.name
        )));
    }
    if doc.nodes.is_empty() && doc.decl.is_none() && doc.doctype.is_none() {
        return Err(IktError::Invalid(if src.trim().is_empty() {
            "输入为空".into()
        } else {
            "输入里没有元素".into()
        }));
    }
    Ok(doc)
}

// ---------------------------------------------------------------- 序列化

fn escape_text(s: &str, out: &mut String) {
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '\r' => out.push_str("&#13;"),
            _ => out.push(ch),
        }
    }
}

fn escape_attr(s: &str, out: &mut String) {
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\n' => out.push_str("&#10;"),
            '\r' => out.push_str("&#13;"),
            '\t' => out.push_str("&#9;"),
            _ => out.push(ch),
        }
    }
}

/// 纯空白文本（元素之间的缩进）不承载数据，序列化时丢掉
fn is_blank(node: &Node, keep_comments: bool) -> bool {
    match node {
        Node::Text(t) => t.trim().is_empty(),
        Node::Comment(_) => !keep_comments,
        _ => false,
    }
}

fn write_node(node: &Node, level: usize, out: &mut String, unit: &str, pretty: bool, spec: &XmlSpec) {
    match node {
        Node::Element(el) => write_element(el, level, out, unit, pretty, spec),
        Node::Text(t) => escape_text(t, out),
        Node::CData(t) => {
            out.push_str("<![CDATA[");
            out.push_str(t);
            out.push_str("]]>");
        }
        Node::Comment(t) => {
            out.push_str("<!--");
            out.push_str(t);
            out.push_str("-->");
        }
        Node::Pi(t) => {
            out.push_str("<?");
            out.push_str(t);
            out.push_str("?>");
        }
    }
}

fn write_element(
    el: &Element,
    level: usize,
    out: &mut String,
    unit: &str,
    pretty: bool,
    spec: &XmlSpec,
) {
    let pad = if pretty { unit.repeat(level) } else { String::new() };
    out.push_str(&pad);
    out.push('<');
    out.push_str(&el.name);
    for (k, v) in &el.attrs {
        out.push(' ');
        out.push_str(k);
        out.push_str("=\"");
        escape_attr(v, out);
        out.push('"');
    }

    let meaningful = el
        .children
        .iter()
        .filter(|c| !is_blank(c, spec.keep_comments))
        .count();
    if meaningful == 0 {
        out.push_str("/>");
        if pretty {
            out.push('\n');
        }
        return;
    }

    out.push('>');
    // 只要元素里有真正的文本或 CDATA，就整体保持行内，避免把文本拆行拆出多余空白
    let inline = el
        .children
        .iter()
        .any(|c| matches!(c, Node::CData(_)) || matches!(c, Node::Text(t) if !t.trim().is_empty()));

    if inline {
        for child in &el.children {
            if is_blank(child, spec.keep_comments) {
                continue;
            }
            // 行内模式不能再让子元素换行
            write_node(child, level, out, unit, false, spec);
        }
        out.push_str("</");
        out.push_str(&el.name);
        out.push('>');
        if pretty {
            out.push('\n');
        }
    } else {
        if pretty {
            out.push('\n');
        }
        for child in &el.children {
            if is_blank(child, spec.keep_comments) {
                continue;
            }
            write_node(child, level + 1, out, unit, pretty, spec);
            if pretty && !out.ends_with('\n') {
                out.push('\n');
            }
        }
        out.push_str(&pad);
        out.push_str("</");
        out.push_str(&el.name);
        out.push('>');
        if pretty {
            out.push('\n');
        }
    }
}

fn tally(
    nodes: &[Node],
    level: usize,
    elements: &mut usize,
    attrs: &mut usize,
    comments: &mut usize,
    depth: &mut usize,
) {
    for node in nodes {
        if let Node::Element(el) = node {
            *elements += 1;
            *attrs += el.attrs.len();
            *comments += el
                .children
                .iter()
                .filter(|c| matches!(c, Node::Comment(_)))
                .count();
            *depth = (*depth).max(level);
            tally(&el.children, level + 1, elements, attrs, comments, depth);
        }
    }
}

fn render(doc: &Doc, spec: &XmlSpec) -> String {
    let pretty = spec.indent > 0;
    let unit = " ".repeat(spec.indent.min(16));
    let mut out = String::new();

    if spec.declaration {
        if let Some((version, encoding)) = &doc.decl {
            out.push_str(&format!(
                "<?xml version=\"{version}\" encoding=\"{encoding}\"?>"
            ));
            if pretty {
                out.push('\n');
            }
        }
    }
    if let Some(dt) = &doc.doctype {
        out.push_str("<!DOCTYPE ");
        out.push_str(dt);
        out.push('>');
        if pretty {
            out.push('\n');
        }
    }

    for node in &doc.nodes {
        if is_blank(node, spec.keep_comments) {
            continue;
        }
        write_node(node, 0, &mut out, &unit, pretty, spec);
        if pretty && !out.ends_with('\n') {
            out.push('\n');
        }
    }

    if pretty {
        while out.ends_with("\n\n") {
            out.pop();
        }
    }
    while out.ends_with('\n') {
        out.pop();
    }
    out
}

fn outcome(output: String, doc: &Doc, renamed: usize, started: Instant) -> XmlOutcome {
    let (mut elements, mut attributes, mut comments, mut depth) = (0, 0, 0, 0);
    tally(&doc.nodes, 1, &mut elements, &mut attributes, &mut comments, &mut depth);
    let root = doc.nodes.iter().find_map(|n| match n {
        Node::Element(el) => Some(el.name.clone()),
        _ => None,
    });
    XmlOutcome {
        output_bytes: output.len(),
        output,
        root,
        elements,
        attributes,
        depth,
        comments,
        renamed,
        elapsed_us: started.elapsed().as_micros(),
    }
}

// ---------------------------------------------------------------- XML → JSON

fn is_number_literal(s: &str) -> bool {
    let b = s.as_bytes();
    if b.is_empty() {
        return false;
    }
    let mut i = 0;
    if b[0] == b'-' {
        i = 1;
        if i == b.len() {
            return false;
        }
    }
    let start = i;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    if i == start {
        return false;
    }
    // 前导零不当作数字：`007` 更像编号
    if b[start] == b'0' && i - start > 1 {
        return false;
    }
    if i < b.len() && b[i] == b'.' {
        i += 1;
        let frac = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if i == frac {
            return false;
        }
    }
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        i += 1;
        if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
            i += 1;
        }
        let exp = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if i == exp {
            return false;
        }
    }
    i == b.len()
}

/// 把文本还原成数字 / 布尔；只在「转成 JSON 再写回来还是同一个写法」时才转换
fn typed(text: &str, numbers: bool) -> Value {
    if !numbers {
        return Value::String(text.to_string());
    }
    match text {
        "true" => return Value::Bool(true),
        "false" => return Value::Bool(false),
        _ => {}
    }
    if !is_number_literal(text) {
        return Value::String(text.to_string());
    }
    match serde_json::from_str::<Value>(text) {
        Ok(v) if v.is_number() && v.to_string() == text => v,
        _ => Value::String(text.to_string()),
    }
}

fn element_value(el: &Element, spec: &XmlSpec) -> Value {
    let mut obj = Map::new();

    for (k, v) in &el.attrs {
        obj.insert(format!("{}{}", spec.attr_prefix, k), Value::String(v.clone()));
    }

    // 同名子元素分组，顺序按第一次出现的位置
    let mut order: Vec<String> = Vec::new();
    let mut buckets: Vec<Vec<&Element>> = Vec::new();
    let mut text = String::new();
    for child in &el.children {
        match child {
            Node::Element(e) => match order.iter().position(|n| n == &e.name) {
                Some(i) => buckets[i].push(e),
                None => {
                    order.push(e.name.clone());
                    buckets.push(vec![e]);
                }
            },
            Node::Text(t) => text.push_str(t),
            Node::CData(t) => text.push_str(t),
            _ => {}
        }
    }

    let text = if spec.trim { text.trim().to_string() } else { text };

    if order.is_empty() && obj.is_empty() {
        return if text.is_empty() {
            Value::String(String::new())
        } else {
            typed(&text, spec.numbers)
        };
    }

    for (name, group) in order.into_iter().zip(buckets) {
        let value = if group.len() == 1 {
            element_value(group[0], spec)
        } else {
            Value::Array(group.into_iter().map(|e| element_value(e, spec)).collect())
        };
        obj.insert(name, value);
    }

    if !text.is_empty() {
        obj.insert(spec.text_key.clone(), typed(&text, spec.numbers));
    }

    Value::Object(obj)
}

pub fn xml_to_json(spec: &XmlSpec, input: &str) -> Result<XmlOutcome> {
    let started = Instant::now();
    let doc = parse(input)?;

    let mut obj = Map::new();
    for node in &doc.nodes {
        if let Node::Element(el) = node {
            let value = element_value(el, spec);
            match obj.get_mut(&el.name) {
                Some(Value::Array(list)) => list.push(value),
                Some(existing) => {
                    let first = existing.take();
                    *existing = Value::Array(vec![first, value]);
                }
                None => {
                    obj.insert(el.name.clone(), value);
                }
            }
        }
    }

    if obj.is_empty() {
        return Err(IktError::Invalid("输入里没有元素".into()));
    }

    let value = Value::Object(obj);
    let output = if spec.indent == 0 {
        serde_json::to_string(&value).map_err(|e| IktError::Internal(e.to_string()))?
    } else if spec.indent == 2 {
        serde_json::to_string_pretty(&value).map_err(|e| IktError::Internal(e.to_string()))?
    } else {
        let pretty =
            serde_json::to_string_pretty(&value).map_err(|e| IktError::Internal(e.to_string()))?;
        reindent_json(&pretty, spec.indent)
    };

    Ok(outcome(output, &doc, 0, started))
}

fn reindent_json(pretty: &str, indent: usize) -> String {
    let unit = " ".repeat(indent.min(16));
    let mut out = String::new();
    let mut level = 0usize;
    for line in pretty.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with('}') || trimmed.starts_with(']') {
            level = level.saturating_sub(1);
        }
        for _ in 0..level {
            out.push_str(&unit);
        }
        out.push_str(trimmed);
        out.push('\n');
        if trimmed.ends_with('{') || trimmed.ends_with('[') {
            level += 1;
        }
    }
    while out.ends_with('\n') {
        out.pop();
    }
    out
}

// ---------------------------------------------------------------- JSON → XML

/// XML Name 的宽松校验：允许非 ASCII（中文是合法的元素名）
fn is_name_char(ch: char, first: bool) -> bool {
    if ch.is_ascii_alphabetic() || ch == '_' || ch == ':' || !ch.is_ascii() {
        return true;
    }
    if first {
        return false;
    }
    ch.is_ascii_digit() || ch == '-' || ch == '.'
}

fn fix_name(name: &str) -> (String, bool) {
    if name.is_empty() {
        return ("_".into(), true);
    }
    let mut out = String::new();
    let mut changed = false;
    for (i, ch) in name.chars().enumerate() {
        if is_name_char(ch, i == 0) {
            out.push(ch);
        } else {
            out.push('_');
            changed = true;
        }
    }
    if out.starts_with(|c: char| c.is_ascii_digit() || c == '-' || c == '.') {
        out.insert(0, '_');
        changed = true;
    }
    // xml 前缀留给规范自己用
    if out.starts_with("xml") {
        out.insert(0, '_');
        changed = true;
    }
    (out, changed)
}

fn scalar_text(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.clone(),
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}

struct Ctx {
    attr_prefix: String,
    text_key: String,
    renamed: usize,
}

fn build_element(name: &str, value: &Value, ctx: &mut Ctx) -> Element {
    let (fixed, changed) = fix_name(name);
    if changed {
        ctx.renamed += 1;
    }
    let mut el = Element {
        name: fixed,
        attrs: Vec::new(),
        children: Vec::new(),
    };

    match value {
        Value::Object(map) => {
            for (k, v) in map {
                if !ctx.attr_prefix.is_empty() {
                    if let Some(rest) = k.strip_prefix(ctx.attr_prefix.as_str()) {
                        if rest.is_empty() {
                            continue;
                        }
                        let (key, changed) = fix_name(rest);
                        if changed {
                            ctx.renamed += 1;
                        }
                        // 属性只能装标量，复杂值退化成紧凑 JSON 字符串
                        el.attrs.push((key, scalar_text(v)));
                        continue;
                    }
                }
                if !ctx.text_key.is_empty() && k == &ctx.text_key {
                    el.children.push(Node::Text(scalar_text(v)));
                    continue;
                }
                match v {
                    Value::Array(items) => {
                        for item in items {
                            el.children.push(Node::Element(build_element(k, item, ctx)));
                        }
                    }
                    other => el.children.push(Node::Element(build_element(k, other, ctx))),
                }
            }
        }
        Value::Array(items) => {
            // 顶层或嵌套数组里没有键名可用，统一用 item 兜住
            for item in items {
                el.children.push(Node::Element(build_element("item", item, ctx)));
            }
        }
        Value::Null => {}
        other => el.children.push(Node::Text(scalar_text(other))),
    }

    el
}

/// 根元素下平铺一组同名字元素（顶层数组的情况）
fn root_from_array(name: &str, child_name: &str, items: &[Value], ctx: &mut Ctx) -> Element {
    let (fixed, changed) = fix_name(name);
    if changed {
        ctx.renamed += 1;
    }
    let mut el = Element {
        name: fixed,
        attrs: Vec::new(),
        children: Vec::new(),
    };
    for item in items {
        el.children
            .push(Node::Element(build_element(child_name, item, ctx)));
    }
    el
}

pub fn json_to_xml(spec: &XmlSpec, input: &str) -> Result<XmlOutcome> {
    let started = Instant::now();
    if input.trim().is_empty() {
        return Err(IktError::Invalid("输入为空".into()));
    }
    let value: Value = serde_json::from_str(input).map_err(|e| {
        IktError::Invalid(format!(
            "JSON 解析失败（第 {} 行第 {} 列）：{}",
            e.line(),
            e.column(),
            e
        ))
    })?;

    let mut ctx = Ctx {
        attr_prefix: spec.attr_prefix.clone(),
        text_key: spec.text_key.clone(),
        renamed: 0,
    };
    let root_name = fix_name(if spec.root.trim().is_empty() {
        "root"
    } else {
        spec.root.trim()
    });

    let root = match &value {
        // 顶层只有一个普通键时直接拿它当根元素，这样 XML → JSON → XML 是原样往返的
        Value::Object(map) if map.len() == 1 => {
            let (k, v) = map.iter().next().expect("只有一个键");
            let special = (!spec.attr_prefix.is_empty() && k.starts_with(spec.attr_prefix.as_str()))
                || (!spec.text_key.is_empty() && k == &spec.text_key);
            if special {
                build_element(&root_name.0, &value, &mut ctx)
            } else {
                match v {
                    // 单键数组：根元素下平铺同名字元素，转回 JSON 时正好还原成数组
                    Value::Array(items) => root_from_array(k, k, items, &mut ctx),
                    other => build_element(k, other, &mut ctx),
                }
            }
        }
        Value::Object(_) => build_element(&root_name.0, &value, &mut ctx),
        Value::Array(items) => root_from_array(&root_name.0, "item", items, &mut ctx),
        other => {
            let mut el = Element {
                name: root_name.0.clone(),
                attrs: Vec::new(),
                children: vec![Node::Text(scalar_text(other))],
            };
            el.children
                .retain(|c| !matches!(c, Node::Text(t) if t.is_empty()));
            el
        }
    };

    let doc = Doc {
        decl: if spec.declaration {
            Some(("1.0".into(), "UTF-8".into()))
        } else {
            None
        },
        doctype: None,
        nodes: vec![Node::Element(root)],
    };
    let renamed = ctx.renamed;
    let output = render(&doc, spec);
    Ok(outcome(output, &doc, renamed, started))
}

// ---------------------------------------------------------------- 格式化 / 压缩

pub fn format(spec: &XmlSpec, input: &str) -> Result<XmlOutcome> {
    let started = Instant::now();
    let doc = parse(input)?;
    let output = render(&doc, spec);
    Ok(outcome(output, &doc, 0, started))
}

pub fn minify(spec: &XmlSpec, input: &str) -> Result<XmlOutcome> {
    let started = Instant::now();
    let doc = parse(input)?;
    let flat = XmlSpec {
        indent: 0,
        ..spec.clone()
    };
    let output = render(&doc, &flat);
    Ok(outcome(output, &doc, 0, started))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec() -> XmlSpec {
        XmlSpec::default()
    }

    #[test]
    fn json_to_xml_basic() {
        let out = json_to_xml(&spec(), r#"{"name":"kun","age":30,"ok":true}"#).unwrap();
        assert!(out.output.contains("<root>"));
        assert!(out.output.contains("<name>kun</name>"));
        assert!(out.output.contains("<age>30</age>"));
        assert!(out
            .output
            .starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
        assert_eq!(out.root.as_deref(), Some("root"));
        assert_eq!(out.elements, 4);
        assert_eq!(out.depth, 2);
    }

    #[test]
    fn json_to_xml_attributes_and_text() {
        let out = json_to_xml(&spec(), r##"{"item":{"@id":"1","#text":"hi"}}"##).unwrap();
        assert!(
            out.output.contains("<item id=\"1\">hi</item>"),
            "{}",
            out.output
        );
        assert_eq!(out.attributes, 1);
    }

    #[test]
    fn json_to_xml_arrays_reuse_parent_name() {
        let out = json_to_xml(&spec(), r#"{"list":[1,2]}"#).unwrap();
        assert!(out.output.contains("<list>1</list>"), "{}", out.output);
        assert!(out.output.contains("<list>2</list>"), "{}", out.output);
    }

    #[test]
    fn json_to_xml_renames_invalid_names() {
        let out = json_to_xml(&spec(), r#"{"a b":1,"1x":2}"#).unwrap();
        assert_eq!(out.renamed, 2);
        assert!(out.output.contains("<a_b>1</a_b>"), "{}", out.output);
        assert!(out.output.contains("<_x>2</_x>"), "{}", out.output);
    }

    #[test]
    fn xml_to_json_basic() {
        let out = xml_to_json(&spec(), "<root><name>kun</name><age>30</age></root>").unwrap();
        let v: Value = serde_json::from_str(&out.output).unwrap();
        assert_eq!(v["root"]["name"], "kun");
        assert_eq!(v["root"]["age"], 30);
        assert_eq!(out.root.as_deref(), Some("root"));
    }

    #[test]
    fn xml_to_json_attributes_and_repeated() {
        let out = xml_to_json(
            &spec(),
            r#"<root><item id="1">a</item><item id="2">b</item></root>"#,
        )
        .unwrap();
        let v: Value = serde_json::from_str(&out.output).unwrap();
        assert_eq!(v["root"]["item"][0]["@id"], "1");
        assert_eq!(v["root"]["item"][0]["#text"], "a");
        assert_eq!(v["root"]["item"][1]["#text"], "b");
    }

    #[test]
    fn numbers_flag_keeps_leading_zeros() {
        let v: Value = serde_json::from_str(
            &xml_to_json(&spec(), "<r><zip>007</zip><n>-12.5</n><b>false</b></r>")
                .unwrap()
                .output,
        )
        .unwrap();
        assert_eq!(v["r"]["zip"], "007");
        assert_eq!(v["r"]["n"], -12.5);
        assert_eq!(v["r"]["b"], false);

        let plain = XmlSpec {
            numbers: false,
            ..Default::default()
        };
        let v: Value =
            serde_json::from_str(&xml_to_json(&plain, "<r><n>12</n></r>").unwrap().output).unwrap();
        assert_eq!(v["r"]["n"], "12");
    }

    #[test]
    fn roundtrip_xml_json_xml() {
        let src = r#"<root><a x="1">t</a><b/><c><d>1</d></c></root>"#;
        let json = xml_to_json(&spec(), src).unwrap().output;
        let back = json_to_xml(&spec(), &json).unwrap().output;
        let again = xml_to_json(&spec(), &back).unwrap().output;
        assert_eq!(
            serde_json::from_str::<Value>(&json).unwrap(),
            serde_json::from_str::<Value>(&again).unwrap()
        );
    }

    #[test]
    fn format_indents_nested_elements() {
        let out = format(&spec(), "<a><b><c>1</c></b></a>").unwrap();
        assert_eq!(out.output, "<a>\n  <b>\n    <c>1</c>\n  </b>\n</a>");
    }

    #[test]
    fn format_keeps_mixed_text_inline() {
        let out = format(&spec(), "<p>hi <b>x</b> there</p>").unwrap();
        assert!(
            out.output.contains("<p>hi <b>x</b> there</p>"),
            "{}",
            out.output
        );
    }

    #[test]
    fn format_keeps_cdata_and_pi() {
        let out = format(
            &spec(),
            r#"<?xml-stylesheet href="a.css"?><r><![CDATA[x<y]]></r>"#,
        )
        .unwrap();
        assert!(out.output.contains("<![CDATA[x<y]]>"), "{}", out.output);
        assert!(out.output.contains("<?xml-stylesheet"), "{}", out.output);
    }

    #[test]
    fn minify_strips_whitespace_but_keeps_text() {
        let src = "<root>\n  <a>1</a>\n  <b> spaced </b>\n</root>";
        let out = minify(&spec(), src).unwrap();
        assert!(
            out.output.ends_with("<root><a>1</a><b> spaced </b></root>"),
            "{}",
            out.output
        );
        assert!(!out.output.contains('\n'));
    }

    #[test]
    fn comments_can_be_dropped() {
        let src = "<r><!-- hi --><a>1</a></r>";
        assert!(format(&spec(), src).unwrap().output.contains("hi"));
        let drop = XmlSpec {
            keep_comments: false,
            ..Default::default()
        };
        assert!(!format(&drop, src).unwrap().output.contains("hi"));
    }

    #[test]
    fn empty_element_is_self_closed() {
        assert!(format(&spec(), "<r><a></a></r>")
            .unwrap()
            .output
            .contains("<a/>"));
    }

    #[test]
    fn escapes_are_restored_and_reapplied() {
        let out = xml_to_json(&spec(), "<r><a>a &amp; b &lt; c</a></r>").unwrap();
        assert!(out.output.contains("a & b < c"), "{}", out.output);
        let back = json_to_xml(&spec(), r#"{"a":"a & b < c"}"#).unwrap();
        assert!(back.output.contains("a &amp; b &lt; c"), "{}", back.output);
    }

    #[test]
    fn declaration_follows_input() {
        assert!(!format(&spec(), "<a/>").unwrap().output.contains("<?xml"));
        let with = format(&spec(), "<?xml version=\"1.0\" encoding=\"utf-8\"?><a/>").unwrap();
        assert!(with.output.starts_with("<?xml version=\"1.0\" encoding=\"utf-8\"?>"));
        let off = XmlSpec {
            declaration: false,
            ..Default::default()
        };
        assert!(!format(&off, "<?xml version=\"1.0\"?><a/>")
            .unwrap()
            .output
            .contains("<?xml"));
    }

    #[test]
    fn invalid_xml_reports_position() {
        let err = format(&spec(), "<a>\n  <b></a>").unwrap_err().to_string();
        assert!(err.starts_with("XML 语法错误（第"), "{err}");
        assert!(!err.contains('\n'), "错误信息不该多行：{err}");

        let err = format(&spec(), "<a><b></b>").unwrap_err().to_string();
        assert!(err.contains("没有闭合"), "{err}");

        let err = format(&spec(), "   ").unwrap_err().to_string();
        assert!(err.contains("输入为空"), "{err}");
    }

    #[test]
    fn json_errors_carry_line_and_column() {
        let err = json_to_xml(&spec(), "{\n  \"a\": }").unwrap_err().to_string();
        assert!(err.contains("JSON 解析失败"), "{err}");
        assert!(err.contains("第 2 行"), "{err}");
    }

    #[test]
    fn contract_is_camel_case() {
        let s = serde_json::to_value(XmlSpec::default()).unwrap();
        for key in [
            "indent",
            "declaration",
            "root",
            "attrPrefix",
            "textKey",
            "trim",
            "numbers",
            "keepComments",
        ] {
            assert!(s.get(key).is_some(), "缺字段 {key}");
        }
        let o = serde_json::to_value(format(&spec(), "<a/>").unwrap()).unwrap();
        for key in [
            "output",
            "outputBytes",
            "root",
            "elements",
            "attributes",
            "depth",
            "comments",
            "renamed",
            "elapsedUs",
        ] {
            assert!(o.get(key).is_some(), "缺字段 {key}");
        }
    }
}
