/**
 * 浏览器兜底实现 —— 仅用于 `pnpm dev` 直接在浏览器里调 UI。
 * 桌面端永远不会走到这里（inTauri() 为真时走 Rust）。
 * 算法参数必须和 crates/ikt-core 保持一致，否则两边行为会分叉。
 */
import type {
  AsciiOutcome,
  AsciiSpec,
  AesOutcome,
  AesSpec,
  HashOutcome,
  JsonOutcome,
  JwtParts,
  RegexGroup,
  RegexMatch,
  RegexOutcome,
  RegexSpec,
  ReplaceOutcome,
  RsaKeyInfo,
  RsaKeyPair,
  RsaOutcome,
  RsaSpec,
  RandomOutcome,
  RandomSpec,
  TimeOutcome,
  TimeSpec,
  UrlOutcome,
  UrlSpec,
  XmlOutcome,
  XmlSpec,
} from "./api";

const te = new TextEncoder();
const td = new TextDecoder();

const KDF_ITERATIONS = 120_000;

function toB64(bytes: Uint8Array, urlSafe = false, noPad = false): string {
  let s = "";
  const chunk = 0x8000;
  for (let i = 0; i < bytes.length; i += chunk) {
    s += String.fromCharCode(...bytes.subarray(i, i + chunk));
  }
  let out = btoa(s);
  if (urlSafe) out = out.replace(/\+/g, "-").replace(/\//g, "_");
  if (noPad) out = out.replace(/=+$/, "");
  return out;
}

function fromB64(input: string): Uint8Array {
  const cleaned = input.replace(/\s/g, "").replace(/-/g, "+").replace(/_/g, "/");
  const padded = cleaned.replace(/=+$/, "") + "=".repeat((4 - (cleaned.replace(/=+$/, "").length % 4)) % 4);
  const bin = atob(padded);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}

function hex(bytes: Uint8Array): string {
  return Array.from(bytes)
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
}

function hexBytes(value: string): Uint8Array {
  const cleaned = value.replace(/[\s:-]/g, "").replace(/^0x/i, "");
  if (cleaned.length % 2 !== 0 || /[^0-9a-f]/i.test(cleaned)) {
    throw new Error("不是合法的十六进制字符串");
  }
  const out = new Uint8Array(cleaned.length / 2);
  for (let i = 0; i < out.length; i++) out[i] = parseInt(cleaned.slice(i * 2, i * 2 + 2), 16);
  return out;
}

function pemOf(der: Uint8Array, label: string): string {
  const b64 = toB64(der);
  const lines = b64.match(/.{1,64}/g) ?? [];
  return `-----BEGIN ${label}-----\n${lines.join("\n")}\n-----END ${label}-----\n`;
}

function pemBody(pem: string): Uint8Array {
  return fromB64(pem.replace(/-----[^-]+-----/g, "").replace(/\s+/g, ""));
}

/** 浏览器只认 SPKI / PKCS#8；PKCS#1 与 v1.5 请到桌面端。 */
async function importRsa(pem: string, use: "encrypt" | "decrypt", hash: string): Promise<CryptoKey> {
  if (/BEGIN RSA (PUBLIC|PRIVATE) KEY/.test(pem)) {
    throw new Error("浏览器预览不支持 PKCS#1 格式的密钥，请在桌面端运行");
  }
  const isPrivate = /PRIVATE KEY/.test(pem);
  if (use === "encrypt" && isPrivate) {
    throw new Error("加密需要公钥，你填的看起来是私钥；浏览器预览里请换成公钥 PEM");
  }
  return crypto.subtle.importKey(
    isPrivate ? "pkcs8" : "spki",
    pemBody(pem) as unknown as BufferSource,
    { name: "RSA-OAEP", hash },
    true,
    [use],
  );
}

async function rsaFingerprint(key: CryptoKey): Promise<string> {
  const spki = new Uint8Array(await crypto.subtle.exportKey("spki", key));
  const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", spki as unknown as BufferSource));
  return `SHA256:${toB64(digest, true, true)}`;
}

function rsaBits(key: CryptoKey): number {
  return (key.algorithm as RsaHashedKeyAlgorithm).modulusLength;
}

function decodeByEncoding(value: string, encoding: string): Uint8Array {
  const v = value.trim();
  if (encoding === "hex") {
    const cleaned = v.replace(/[\s:-]/g, "");
    const out = new Uint8Array(Math.floor(cleaned.length / 2));
    for (let i = 0; i < out.length; i++) out[i] = parseInt(cleaned.slice(i * 2, i * 2 + 2), 16);
    return out;
  }
  if (encoding === "base64") return fromB64(v);
  return te.encode(v);
}

async function deriveAes(password: string, salt: Uint8Array): Promise<CryptoKey> {
  const base = await crypto.subtle.importKey("raw", te.encode(password), "PBKDF2", false, [
    "deriveKey",
  ]);
  return crypto.subtle.deriveKey(
    { name: "PBKDF2", salt: salt as unknown as BufferSource, iterations: KDF_ITERATIONS, hash: "SHA-256" },
    base,
    { name: "AES-GCM", length: 256 },
    false,
    ["encrypt", "decrypt"],
  );
}

function locate(src: string, err: unknown): { line: number; column: number; snippet: string } {
  const msg = String(err);
  let offset = -1;
  // 逐步加长前缀，找到第一个「不是意外结束」的失败点
  const limit = Math.min(src.length, 4000);
  for (let i = 1; i <= limit; i++) {
    try {
      JSON.parse(src.slice(0, i));
    } catch (e) {
      const m = String(e);
      if (!/end of (JSON )?(input|data)|Unterminated/i.test(m)) {
        offset = i - 1;
        break;
      }
    }
  }
  if (offset < 0) offset = Math.min(src.length, 0);
  const before = src.slice(0, offset);
  const line = before.split("\n").length;
  const column = offset - before.lastIndexOf("\n");
  const snippet = src.split("\n")[line - 1]?.slice(0, 200) ?? "";
  void msg;
  return { line, column, snippet };
}

function sortValue(v: unknown): unknown {
  if (Array.isArray(v)) return v.map(sortValue);
  if (v && typeof v === "object") {
    const src = v as Record<string, unknown>;
    const out: Record<string, unknown> = {};
    for (const k of Object.keys(src).sort()) out[k] = sortValue(src[k]);
    return out;
  }
  return v;
}

function shape(v: unknown): { nodes: number; depth: number; objects: number; arrays: number } {
  let nodes = 0;
  let depth = 0;
  let objects = 0;
  let arrays = 0;
  const walk = (x: unknown, d: number) => {
    nodes += 1;
    if (d > depth) depth = d;
    if (Array.isArray(x)) {
      arrays += 1;
      x.forEach((c) => walk(c, d + 1));
    } else if (x && typeof x === "object") {
      objects += 1;
      Object.values(x as Record<string, unknown>).forEach((c) => walk(c, d + 1));
    }
  };
  walk(v, 1);
  return { nodes, depth, objects, arrays };
}

/** 粗略还原 `x` 标志：去掉字符类外、未转义的空白 */
function stripExtended(src: string): string {
  let out = "";
  let inClass = false;
  for (let i = 0; i < src.length; i += 1) {
    const c = src[i];
    if (c === "\\") {
      out += c + (src[i + 1] ?? "");
      i += 1;
      continue;
    }
    if (inClass) {
      out += c;
      if (c === "]") inClass = false;
      continue;
    }
    if (c === "[") {
      inClass = true;
      out += c;
      continue;
    }
    if (/\s/.test(c)) continue;
    out += c;
  }
  return out;
}

/** 按出现顺序取捕获组名字，未命名的位置填 null（用来对齐 JS 的组序号） */
function groupNames(src: string): (string | null)[] {
  const names: (string | null)[] = [];
  let inClass = false;
  for (let i = 0; i < src.length; i += 1) {
    const c = src[i];
    if (c === "\\") {
      i += 1;
      continue;
    }
    if (inClass) {
      if (c === "]") inClass = false;
      continue;
    }
    if (c === "[") {
      inClass = true;
      continue;
    }
    if (c !== "(") continue;
    if (src[i + 1] !== "?") {
      names.push(null);
      continue;
    }
    if (src[i + 2] === "<" && (src[i + 3] === "=" || src[i + 3] === "!")) continue;
    if (src[i + 2] === "<") {
      const end = src.indexOf(">", i + 3);
      names.push(end > 0 ? src.slice(i + 3, end) : null);
      continue;
    }
    if (src[i + 2] === "P" && src[i + 3] === "<") {
      const end = src.indexOf(">", i + 4);
      names.push(end > 0 ? src.slice(i + 4, end) : null);
    }
    // (?: (?= (?! 不占组号，跳过
  }
  return names;
}

function lineStartOffsets(text: string): number[] {
  const starts = [0];
  for (let i = 0; i < text.length; i += 1) if (text[i] === "\n") starts.push(i + 1);
  return starts;
}

function locateLine(starts: number[], offset: number): [number, number] {
  let lo = 0;
  let hi = starts.length - 1;
  while (lo < hi) {
    const mid = (lo + hi + 1) >> 1;
    if (starts[mid] <= offset) lo = mid;
    else hi = mid - 1;
  }
  return [lo + 1, offset - starts[lo] + 1];
}

function buildJsRegex(pattern: string, spec: Partial<RegexSpec>): { re: RegExp; names: (string | null)[] } {
  if (!pattern.trim()) throw new Error("正则表达式为空");
  // Rust 的 (?P<name>...) 在 JS 里写作 (?<name>...)
  const normalized = pattern.replace(/\(\?P</g, "(?<");
  const src = spec.extended ? stripExtended(normalized) : normalized;
  let flags = "g";
  if (spec.ignoreCase) flags += "i";
  if (spec.multiLine) flags += "m";
  if (spec.dotAll) flags += "s";
  if (spec.unicode !== false) flags += "u";
  try {
    return { re: new RegExp(src, flags), names: groupNames(src) };
  } catch (e) {
    const raw = String(e).replace(/^SyntaxError:\s*/, "");
    const cleaned = raw.replace(/^Invalid regular expression:\s*\/[\s\S]*?\/[a-z]*:\s*/, "");
    throw new Error(`正则语法错误：${cleaned}`);
  }
}

/** 替换串展开：支持 $0 $1 $& ${1} $name ${name} $<name> 与字面 $$ */
function expandTemplate(tpl: string, m: RegExpExecArray): string {
  return tpl.replace(
    /\$(\$|&|\d{1,2}|\{[^}]*\}|<[^>]*>|[A-Za-z_]\w*)/g,
    (_all, token: string) => {
      if (token === "$") return "$";
      if (token === "&" || token === "0") return m[0];
      if (token.startsWith("{")) {
        const key = token.slice(1, -1);
        const v = /^\d+$/.test(key) ? m[Number(key)] : m.groups?.[key];
        return v ?? "";
      }
      if (token.startsWith("<")) return m.groups?.[token.slice(1, -1)] ?? "";
      if (/^\d+$/.test(token)) return m[Number(token)] ?? "";
      return m.groups?.[token] ?? "";
    },
  );
}

// ---------------------------------------------------------------- XML

type XmlStats = { elements: number; attributes: number; depth: number; comments: number; renamed: number };

function xmlParse(src: string): { doc: Document; decl: { version: string; encoding: string } | null } {
  if (!src.trim()) throw new Error("输入为空");
  const doc = new DOMParser().parseFromString(src, "application/xml");
  const bad = doc.getElementsByTagName("parsererror")[0];
  if (bad) {
    const raw = (bad.textContent ?? "").replace(/\s+/g, " ").trim();
    // Chrome 会把「错误 + 半成品页面」一起塞进 textContent，这里只留错误那一句
    const m = /error on line (\d+) at column (\d+):\s*(.*?)(?:\s*Below is|$)/i.exec(raw);
    const detail = m
      ? `（第 ${m[1]} 行第 ${m[2]} 列）：${m[3].trim()}`
      : `：${raw.replace(/^This page contains the following errors:?/i, "").trim()}`;
    throw new Error(`XML 语法错误${detail}`);
  }
  // DOMParser 会丢掉 XML 声明，从原文里捞回来
  const match = /^\s*<\?xml\s+([^?]*)\?>/.exec(src);
  let decl: { version: string; encoding: string } | null = null;
  if (match) {
    const version = /version\s*=\s*["']([^"']*)["']/.exec(match[1])?.[1] ?? "1.0";
    const encoding = /encoding\s*=\s*["']([^"']*)["']/.exec(match[1])?.[1] ?? "UTF-8";
    decl = { version, encoding };
  }
  if (!doc.documentElement) throw new Error("输入里没有元素");
  return { doc, decl };
}

function xmlEscText(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/\r/g, "&#13;");
}

function xmlEscAttr(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/\n/g, "&#10;")
    .replace(/\r/g, "&#13;")
    .replace(/\t/g, "&#9;");
}

/** 纯空白文本（元素之间的缩进）不承载数据，序列化时丢掉 */
function xmlIsBlank(n: Node, keepComments: boolean): boolean {
  if (n.nodeType === 3) return (n.nodeValue ?? "").trim() === "";
  if (n.nodeType === 8) return !keepComments;
  return false;
}

function xmlSerializeNode(
  n: Node,
  level: number,
  unit: string,
  pretty: boolean,
  spec: XmlSpec,
  stats: XmlStats,
): string {
  switch (n.nodeType) {
    case 1: {
      const el = n as Element;
      stats.elements += 1;
      stats.attributes += el.attributes.length;
      // 根元素不缩进，但深度算 1
      stats.depth = Math.max(stats.depth, level + 1);
      const kids = Array.from(el.childNodes);
      if (spec.keepComments) {
        stats.comments += kids.filter((k) => k.nodeType === 8).length;
      }
      const meaningful = kids.filter((k) => !xmlIsBlank(k, spec.keepComments));
      const pad = pretty ? unit.repeat(level) : "";
      let out = `${pad}<${el.tagName}`;
      for (const a of Array.from(el.attributes)) out += ` ${a.name}="${xmlEscAttr(a.value)}"`;
      if (meaningful.length === 0) return out + (pretty ? "/>\n" : "/>");
      out += ">";
      // 有真正的文本或 CDATA 时整体保持行内，避免拆行拆出多余空白
      const inline = kids.some(
        (k) => k.nodeType === 4 || (k.nodeType === 3 && (k.nodeValue ?? "").trim() !== ""),
      );
      if (inline) {
        for (const k of meaningful) out += xmlSerializeNode(k, level, unit, false, spec, stats);
        out += `</${el.tagName}>`;
        return pretty ? out + "\n" : out;
      }
      if (pretty) out += "\n";
      for (const k of meaningful) {
        out += xmlSerializeNode(k, level + 1, unit, pretty, spec, stats);
        if (pretty && !out.endsWith("\n")) out += "\n";
      }
      out += `${pad}</${el.tagName}>`;
      return pretty ? out + "\n" : out;
    }
    case 3:
      return xmlEscText(n.nodeValue ?? "");
    case 4:
      return `<![CDATA[${n.nodeValue ?? ""}]]>`;
    case 8:
      return `<!--${n.nodeValue ?? ""}-->`;
    case 7:
      return `<?${n.nodeName} ${n.nodeValue ?? ""}?>`;
    default:
      return "";
  }
}

function xmlSerializeDoc(
  doc: Document,
  spec: XmlSpec,
  decl: { version: string; encoding: string } | null,
): { output: string; stats: XmlStats } {
  const pretty = spec.indent > 0;
  const unit = " ".repeat(Math.min(spec.indent, 16));
  const stats: XmlStats = { elements: 0, attributes: 0, depth: 0, comments: 0, renamed: 0 };
  let out = "";
  if (spec.declaration && decl) {
    out += `<?xml version="${decl.version}" encoding="${decl.encoding}"?>`;
    if (pretty) out += "\n";
  }
  const dt = doc.doctype;
  if (dt) {
    let s = `<!DOCTYPE ${dt.name}`;
    if (dt.publicId) s += ` PUBLIC "${dt.publicId}"`;
    if (dt.systemId) s += dt.publicId ? ` "${dt.systemId}"` : ` SYSTEM "${dt.systemId}"`;
    out += `${s}>`;
    if (pretty) out += "\n";
  }
  for (const node of Array.from(doc.childNodes)) {
    if (node.nodeType === 10) continue;
    if (xmlIsBlank(node, spec.keepComments)) continue;
    out += xmlSerializeNode(node, 0, unit, pretty, spec, stats);
    if (pretty && !out.endsWith("\n")) out += "\n";
  }
  while (out.endsWith("\n")) out = out.slice(0, -1);
  return { output: out, stats };
}

function xmlIsNumberLiteral(s: string): boolean {
  return /^-?(0|[1-9]\d*)(\.\d+)?([eE][+-]?\d+)?$/.test(s);
}

/** 只在「转成 JSON 再写回来还是同一个写法」时才转换，避免 007 / 1e3 变形 */
function xmlTyped(text: string, numbers: boolean): unknown {
  if (!numbers) return text;
  if (text === "true") return true;
  if (text === "false") return false;
  if (!xmlIsNumberLiteral(text)) return text;
  const n = Number(text);
  if (!Number.isFinite(n) || String(n) !== text) return text;
  return n;
}

function xmlElementValue(el: Element, spec: XmlSpec): unknown {
  const obj: Record<string, unknown> = {};
  for (const a of Array.from(el.attributes)) obj[spec.attrPrefix + a.name] = a.value;

  const order: string[] = [];
  const buckets: Element[][] = [];
  let text = "";
  for (const child of Array.from(el.childNodes)) {
    if (child.nodeType === 1) {
      const kid = child as Element;
      const i = order.indexOf(kid.tagName);
      if (i < 0) {
        order.push(kid.tagName);
        buckets.push([kid]);
      } else buckets[i].push(kid);
    } else if (child.nodeType === 3 || child.nodeType === 4) {
      text += child.nodeValue ?? "";
    }
  }
  if (spec.trim) text = text.trim();

  if (order.length === 0 && Object.keys(obj).length === 0) {
    return text === "" ? "" : xmlTyped(text, spec.numbers);
  }
  for (let i = 0; i < order.length; i += 1) {
    const group = buckets[i];
    obj[order[i]] =
      group.length === 1 ? xmlElementValue(group[0], spec) : group.map((e) => xmlElementValue(e, spec));
  }
  if (text !== "") obj[spec.textKey] = xmlTyped(text, spec.numbers);
  return obj;
}

function xmlFixName(name: string): { name: string; changed: boolean } {
  if (!name) return { name: "_", changed: true };
  let out = "";
  let changed = false;
  for (let i = 0; i < name.length; i += 1) {
    const ch = name[i];
    const first = i === 0;
    const ok = /[A-Za-z_:]/.test(ch) || ch.charCodeAt(0) >= 128 || (!first && /[0-9.-]/.test(ch));
    if (ok) out += ch;
    else {
      out += "_";
      changed = true;
    }
  }
  if (/^[0-9.-]/.test(out) || out.startsWith("xml")) {
    out = `_${out}`;
    changed = true;
  }
  return { name: out, changed };
}

function xmlScalar(v: unknown): string {
  if (v === null || v === undefined) return "";
  if (typeof v === "string") return v;
  if (typeof v === "object") return JSON.stringify(v);
  return String(v);
}

function xmlBuildElement(
  doc: Document,
  name: string,
  value: unknown,
  spec: XmlSpec,
  stats: XmlStats,
): Element {
  const fixed = xmlFixName(name);
  if (fixed.changed) stats.renamed += 1;
  const el = doc.createElement(fixed.name);

  if (Array.isArray(value)) {
    for (const item of value) el.appendChild(xmlBuildElement(doc, "item", item, spec, stats));
    return el;
  }
  if (value && typeof value === "object") {
    for (const [k, v] of Object.entries(value as Record<string, unknown>)) {
      if (spec.attrPrefix && k.startsWith(spec.attrPrefix)) {
        const rest = k.slice(spec.attrPrefix.length);
        if (!rest) continue;
        const attr = xmlFixName(rest);
        if (attr.changed) stats.renamed += 1;
        el.setAttribute(attr.name, xmlScalar(v));
        continue;
      }
      if (spec.textKey && k === spec.textKey) {
        el.appendChild(doc.createTextNode(xmlScalar(v)));
        continue;
      }
      if (Array.isArray(v)) {
        for (const item of v) el.appendChild(xmlBuildElement(doc, k, item, spec, stats));
      } else {
        el.appendChild(xmlBuildElement(doc, k, v, spec, stats));
      }
    }
    return el;
  }
  if (value !== null && value !== undefined) {
    el.appendChild(doc.createTextNode(xmlScalar(value)));
  }
  return el;
}

function xmlJsonToXml(spec: XmlSpec, input: string): XmlOutcome {
  let value: unknown;
  try {
    value = JSON.parse(input);
  } catch (e) {
    throw new Error(`JSON 解析失败：${String(e).replace(/^SyntaxError:\s*/, "")}`);
  }
  const doc = document.implementation.createDocument(null, null, null);
  const stats: XmlStats = { elements: 0, attributes: 0, depth: 0, comments: 0, renamed: 0 };
  const rootName = spec.root.trim() || "root";
  const isObject = value !== null && typeof value === "object" && !Array.isArray(value);
  const entries = isObject ? Object.entries(value as Record<string, unknown>) : [];
  const single =
    entries.length === 1 &&
    !(spec.attrPrefix && entries[0][0].startsWith(spec.attrPrefix)) &&
    !(spec.textKey && entries[0][0] === spec.textKey);

  let root: Element;
  if (single) {
    const [k, v] = entries[0];
    const fixed = xmlFixName(k);
    if (fixed.changed) stats.renamed += 1;
    root = doc.createElement(fixed.name);
    if (Array.isArray(v)) {
      for (const item of v) root.appendChild(xmlBuildElement(doc, k, item, spec, stats));
    } else {
      root = xmlBuildElement(doc, k, v, spec, stats);
    }
  } else if (isObject) {
    root = xmlBuildElement(doc, rootName, value, spec, stats);
  } else if (Array.isArray(value)) {
    const fixed = xmlFixName(rootName);
    root = doc.createElement(fixed.name);
    for (const item of value) root.appendChild(xmlBuildElement(doc, "item", item, spec, stats));
  } else {
    const fixed = xmlFixName(rootName);
    root = doc.createElement(fixed.name);
    const text = xmlScalar(value);
    if (text) root.appendChild(doc.createTextNode(text));
  }
  doc.appendChild(root);

  const { output, stats: counted } = xmlSerializeDoc(
    doc,
    spec,
    spec.declaration ? { version: "1.0", encoding: "UTF-8" } : null,
  );
  return {
    output,
    outputBytes: te.encode(output).length,
    root: root.tagName,
    elements: counted.elements,
    attributes: counted.attributes,
    depth: counted.depth,
    comments: counted.comments,
    renamed: stats.renamed,
    elapsedUs: 0,
  };
}

function xmlToXmlOutcome(
  doc: Document,
  decl: { version: string; encoding: string } | null,
  spec: XmlSpec,
  started: number,
): XmlOutcome {
  const { output, stats } = xmlSerializeDoc(doc, spec, decl);
  return {
    output,
    outputBytes: te.encode(output).length,
    root: doc.documentElement?.tagName ?? null,
    elements: stats.elements,
    attributes: stats.attributes,
    depth: stats.depth,
    comments: stats.comments,
    renamed: 0,
    elapsedUs: Math.round((performance.now() - started) * 1000),
  };
}

// ---------------------------------------------------------------- 时间

function tzOffsetSeconds(tz: string): number {
  const raw = tz.trim();
  if (!raw) throw new Error("时区不能为空");
  const upper = raw.toUpperCase();
  if (["Z", "UTC", "GMT", "+00:00", "0"].includes(upper)) return 0;
  const body = upper.startsWith("UTC") ? upper.slice(3) : upper.startsWith("GMT") ? upper.slice(3) : upper;
  const sign = body.startsWith("-") ? -1 : 1;
  const digits = body.replace(/[^0-9]/g, "");
  if (!digits) throw new Error(`时区写法不对：${tz}`);
  let h: number;
  let m: number;
  if (digits.length <= 2) {
    h = Number(digits);
    m = 0;
  } else if (digits.length === 3) {
    h = Number(digits[0]);
    m = Number(digits.slice(1));
  } else {
    h = Number(digits.slice(0, 2));
    m = Number(digits.slice(2, 4));
  }
  if (h > 23 || m > 59) throw new Error(`时区写法不对：${tz}`);
  return sign * (h * 3600 + m * 60);
}

const pad2 = (n: number) => String(n).padStart(2, "0");

function tzLabel(offsetSec: number): string {
  const sign = offsetSec < 0 ? "-" : "+";
  const abs = Math.abs(offsetSec);
  return `${sign}${pad2(Math.floor(abs / 3600))}:${pad2(Math.floor((abs % 3600) / 60))}`;
}

function isoParts(ms: number, offsetSec: number) {
  const d = new Date(ms + offsetSec * 1000);
  return {
    d,
    date: `${d.getUTCFullYear()}-${pad2(d.getUTCMonth() + 1)}-${pad2(d.getUTCDate())}`,
    time: `${pad2(d.getUTCHours())}:${pad2(d.getUTCMinutes())}:${pad2(d.getUTCSeconds())}`,
    ms: d.getUTCMilliseconds(),
  };
}

function isoWeek(d: Date): number {
  const t = new Date(Date.UTC(d.getUTCFullYear(), d.getUTCMonth(), d.getUTCDate()));
  const day = t.getUTCDay() || 7;
  t.setUTCDate(t.getUTCDate() + 4 - day);
  const yearStart = Date.UTC(t.getUTCFullYear(), 0, 1);
  return Math.ceil(((t.getTime() - yearStart) / 86_400_000 + 1) / 7);
}

function dayOfYear(d: Date): number {
  const start = Date.UTC(d.getUTCFullYear(), 0, 1);
  const cur = Date.UTC(d.getUTCFullYear(), d.getUTCMonth(), d.getUTCDate());
  return Math.floor((cur - start) / 86_400_000) + 1;
}

function relativeCn(targetSec: number, nowSec: number): string {
  const diff = targetSec - nowSec;
  const abs = Math.abs(diff);
  const suffix = diff >= 0 ? "后" : "前";
  if (abs < 1) return "刚刚";
  if (abs < 60) return `${Math.floor(abs)} 秒${suffix}`;
  const minutes = abs / 60;
  if (minutes < 60) return `${Math.floor(minutes)} 分钟${suffix}`;
  const hours = minutes / 60;
  if (hours < 24) return `${Math.floor(hours)} 小时${suffix}`;
  const days = hours / 24;
  if (days < 30) return `${Math.floor(days)} 天${suffix}`;
  const months = days / 30;
  if (months < 12) return `${Math.floor(months)} 个月${suffix}`;
  return `${(months / 12).toFixed(1)} 年${suffix}`;
}

const WEEKDAY = ["周日", "周一", "周二", "周三", "周四", "周五", "周六"];

function buildTime(ms: number, offsetSec: number, direction: string, unit: string, started: number): TimeOutcome {
  const local = isoParts(ms, offsetSec);
  const utc = isoParts(ms, 0);
  const seconds = Math.floor(ms / 1000);
  const subMs = ((ms % 1000) + 1000) % 1000;
  return {
    direction: direction as TimeOutcome["direction"],
    unit,
    seconds,
    millis: seconds * 1000 + subMs,
    micros: seconds * 1_000_000 + subMs * 1000,
    nanos: seconds * 1_000_000_000 + subMs * 1_000_000,
    utcIso: `${utc.date}T${utc.time}.${String(utc.ms).padStart(3, "0")}Z`,
    utcText: `${utc.date} ${utc.time} UTC`,
    localIso: `${local.date}T${local.time}.${String(local.ms).padStart(3, "0")}${tzLabel(offsetSec)}`,
    localText: `${local.date} ${local.time}`,
    tzLabel: tzLabel(offsetSec),
    weekday: WEEKDAY[local.d.getUTCDay()],
    dayOfYear: dayOfYear(local.d),
    week: isoWeek(local.d),
    relative: relativeCn(seconds, Math.floor(Date.now() / 1000)),
    nowMs: Date.now(),
    elapsedUs: Math.round((performance.now() - started) * 1000),
  };
}

function detectUnit(digits: number): string {
  if (digits <= 11) return "s";
  if (digits <= 14) return "ms";
  if (digits <= 17) return "us";
  return "ns";
}

const UNIT_SCALE: Record<string, bigint> = {
  s: 1_000_000_000n,
  ms: 1_000_000n,
  us: 1_000n,
  ns: 1n,
};

function numericToMs(raw: string, unit: string): number {
  const sign = raw.startsWith("-") ? -1n : 1n;
  const body = raw.replace(/^[+-]/, "");
  const [intPart, fracPart = ""] = body.split(".");
  if (!intPart) throw new Error("时间戳格式不对");
  const scale = UNIT_SCALE[unit];
  let nanos = BigInt(intPart) * scale;
  const fracDigits = unit === "s" ? 9 : unit === "ms" ? 6 : unit === "us" ? 3 : 0;
  if (fracPart && fracDigits > 0) {
    nanos += BigInt(fracPart.slice(0, fracDigits).padEnd(fracDigits, "0"));
  }
  nanos *= sign;
  return Number(nanos / 1_000_000n);
}

/** 认得出的写法：ISO / RFC3339 / `YYYY-MM-DD HH:MM:SS` / 斜杠 / 紧凑数字 */
/** 构造 UTC 时间戳，同时拒绝 2026-13-45 这种会被 Date 静默进位的写法 */
function makeUtc(
  y: number,
  mo: number,
  d: number,
  h: number,
  mi: number,
  sec: number,
  msec = 0,
): number {
  const t = Date.UTC(y, mo - 1, d, h, mi, sec, msec);
  const dt = new Date(t);
  if (
    dt.getUTCFullYear() !== y ||
    dt.getUTCMonth() !== mo - 1 ||
    dt.getUTCDate() !== d ||
    dt.getUTCHours() !== h ||
    dt.getUTCMinutes() !== mi ||
    dt.getUTCSeconds() !== sec
  ) {
    throw new Error("这个日期不存在");
  }
  return t;
}

function parseDateTime(raw: string, offsetSec: number): number {
  const withOffset = raw.match(
    /^(\d{4})-(\d{2})-(\d{2})[T ](\d{2}):(\d{2})(?::(\d{2})(?:\.(\d{1,9}))?)?\s*(Z|[+-]\d{2}:?\d{2})$/i,
  );
  if (withOffset) {
    const [, y, mo, d, h, mi, s = "0", frac = "", tz] = withOffset;
    const base = makeUtc(+y, +mo, +d, +h, +mi, +s, +frac.slice(0, 3).padEnd(3, "0"));
    const off = /^[Zz]$/.test(tz) ? 0 : tzOffsetSeconds(tz);
    return base - off * 1000;
  }
  const naive = raw.match(
    /^(\d{4})[-/](\d{2})[-/](\d{2})(?:[T ](\d{2}):(\d{2})(?::(\d{2})(?:\.(\d{1,9}))?)?)?[Zz]?$/,
  );
  if (naive) {
    const [, y, mo, d, h = "0", mi = "0", s = "0", frac = ""] = naive;
    return makeUtc(+y, +mo, +d, +h, +mi, +s, +frac.slice(0, 3).padEnd(3, "0")) - offsetSec * 1000;
  }
  const compact = raw.match(/^(\d{4})(\d{2})(\d{2})(?:(\d{2})(\d{2})(\d{2}))?$/);
  if (compact) {
    const [, y, mo, d, h = "0", mi = "0", s = "0"] = compact;
    return makeUtc(+y, +mo, +d, +h, +mi, +s) - offsetSec * 1000;
  }
  throw new Error(`认不出这个时间：${raw}（可试 2026-11-17 14:24:00 或 2026-11-17T14:24:00+08:00）`);
}

function timeConvert(spec: TimeSpec, input: string, started: number): TimeOutcome {
  const raw = input.trim();
  if (!raw) throw new Error("输入为空");
  const offsetSec = tzOffsetSeconds(spec.tz);
  const body = raw.replace(/^[+-]/, "");
  const isNumeric =
    body.length > 0 && /^[0-9.]+$/.test(body) && /[0-9]/.test(body) && (body.match(/\./g) ?? []).length <= 1;
  if (isNumeric) {
    const intDigits = (body.split(".")[0] ?? "").replace(/^0+/, "").length;
    const unit = spec.unit === "" || spec.unit === "auto" ? detectUnit(intDigits || 1) : spec.unit;
    if (!["s", "ms", "us", "ns"].includes(unit)) throw new Error(`单位只能是 auto / s / ms / us / ns：${unit}`);
    return buildTime(numericToMs(raw, unit), offsetSec, "toDate", unit, started);
  }
  return buildTime(parseDateTime(raw, offsetSec), offsetSec, "toTimestamp", "s", started);
}

// ---------------------------------------------------------------- 随机

const NANOID_ALPHABET = "useandom-26T198340PX75pxJACKVERYMINDBUSHWOLF_GQZbfghjklqvwyzrict";
const CROCKFORD = "0123456789ABCDEFGHJKMNPQRSTVWXYZ";
const LOWER = "abcdefghijklmnopqrstuvwxyz";
const UPPER = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
const DIGITS = "0123456789";
const SYMBOLS = "!@#$%^&*()-_=+[]{};:,.?/";
const AMBIGUOUS = "0O1lI|`'\"";

function randInt(n: number): number {
  if (n <= 1) return 0;
  const limit = Math.floor(0x1_0000_0000 / n) * n;
  const buf = new Uint32Array(1);
  for (;;) {
    crypto.getRandomValues(buf);
    if (buf[0] < limit) return buf[0] % n;
  }
}

function randBytes(n: number): Uint8Array {
  const out = new Uint8Array(n);
  crypto.getRandomValues(out);
  return out;
}

function randomHex(bytes: Uint8Array): string {
  return Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");
}

function uniqueChars(s: string): string[] {
  return Array.from(new Set(Array.from(s)));
}

function uuidV4(): string {
  const b = randBytes(16);
  b[6] = (b[6] & 0x0f) | 0x40;
  b[8] = (b[8] & 0x3f) | 0x80;
  const h = randomHex(b);
  return `${h.slice(0, 8)}-${h.slice(8, 12)}-${h.slice(12, 16)}-${h.slice(16, 20)}-${h.slice(20)}`;
}

function uuidV7(): string {
  const ms = Date.now();
  const b = randBytes(16);
  for (let i = 0; i < 6; i += 1) b[i] = (ms >> (8 * (5 - i))) & 0xff;
  b[6] = (b[6] & 0x0f) | 0x70;
  b[8] = (b[8] & 0x3f) | 0x80;
  const h = randomHex(b);
  return `${h.slice(0, 8)}-${h.slice(8, 12)}-${h.slice(12, 16)}-${h.slice(16, 20)}-${h.slice(20)}`;
}

function ulid(): string {
  const ms = BigInt(Date.now()) & 0xffff_ffff_ffffn;
  let tail = 0n;
  for (const byte of randBytes(10)) tail = (tail << 8n) | BigInt(byte);
  const value = (ms << 80n) | tail;
  let out = "";
  for (let i = 25; i >= 0; i -= 1) {
    out += CROCKFORD[Number((value >> BigInt(5 * i)) & 31n)];
  }
  return out;
}

function nanoid(len: number): string {
  const chars = Array.from(NANOID_ALPHABET);
  return Array.from({ length: len }, () => chars[randInt(chars.length)]).join("");
}

function passwordAlphabet(spec: RandomSpec): string[] {
  let pool = "";
  if (spec.lower) pool += LOWER;
  if (spec.upper) pool += UPPER;
  if (spec.digits) pool += DIGITS;
  if (spec.symbols) pool += SYMBOLS;
  if (!pool) throw new Error("至少要勾选一类字符");
  let chars = uniqueChars(pool);
  if (spec.excludeAmbiguous) chars = chars.filter((c) => !AMBIGUOUS.includes(c));
  return chars;
}

function password(spec: RandomSpec, len: number): string {
  const alphabet = passwordAlphabet(spec);
  const classes: [string, boolean][] = [
    [LOWER, spec.lower],
    [UPPER, spec.upper],
    [DIGITS, spec.digits],
    [SYMBOLS, spec.symbols],
  ];
  const required: string[] = [];
  for (const [cls, on] of classes) {
    if (!on) continue;
    const filtered = Array.from(cls).filter((c) => !spec.excludeAmbiguous || !AMBIGUOUS.includes(c));
    if (filtered.length) required.push(filtered[randInt(filtered.length)]);
  }
  if (len < required.length) throw new Error(`长度至少要 ${required.length} 位才能覆盖勾选的字符类型`);
  const chars = Array.from({ length: len - required.length }, () => alphabet[randInt(alphabet.length)]);
  const all = [...chars, ...required];
  for (let i = all.length - 1; i > 0; i -= 1) {
    const j = randInt(i + 1);
    [all[i], all[j]] = [all[j], all[i]];
  }
  return all.join("");
}

function randomText(spec: RandomSpec, len: number): { text: string; size: number } {
  const alphabet = spec.alphabet.trim()
    ? uniqueChars(spec.alphabet.trim())
    : uniqueChars(`${LOWER}${UPPER}${DIGITS}`);
  if (!alphabet.length) throw new Error("字符集为空");
  const text = Array.from({ length: len }, () => alphabet[randInt(alphabet.length)]).join("");
  return { text, size: alphabet.length };
}

function randomGenerate(spec: RandomSpec, started: number): RandomOutcome {
  const count = Math.min(Math.max(spec.count, 1), 1000);
  const length = Math.min(Math.max(spec.length, 1), 4096);
  const items: string[] = [];
  let bits = 0;
  let alphabetSize = 0;
  switch (spec.kind) {
    case "uuid4":
      bits = 122;
      for (let i = 0; i < count; i += 1) items.push(uuidV4());
      break;
    case "uuid7":
      bits = 74;
      for (let i = 0; i < count; i += 1) items.push(uuidV7());
      break;
    case "ulid":
      bits = 80;
      for (let i = 0; i < count; i += 1) items.push(ulid());
      break;
    case "nanoid":
      alphabetSize = Array.from(NANOID_ALPHABET).length;
      bits = length * Math.log2(alphabetSize);
      for (let i = 0; i < count; i += 1) items.push(nanoid(length));
      break;
    case "password":
      alphabetSize = passwordAlphabet(spec).length;
      bits = length * Math.log2(alphabetSize);
      for (let i = 0; i < count; i += 1) items.push(password(spec, length));
      break;
    case "text": {
      const probe = randomText(spec, length);
      alphabetSize = probe.size;
      bits = length * Math.log2(probe.size);
      items.push(probe.text);
      for (let i = 1; i < count; i += 1) items.push(randomText(spec, length).text);
      break;
    }
    case "bytes": {
      const n = Math.min(Math.max(spec.bytesLen, 1), 4096);
      bits = n * 8;
      for (let i = 0; i < count; i += 1) {
        const b = randBytes(n);
        if (spec.bytesEncoding === "hex") items.push(randomHex(b));
        else if (spec.bytesEncoding === "base64") items.push(toB64(b));
        else throw new Error(`不支持的输出编码：${spec.bytesEncoding}`);
      }
      break;
    }
    case "int":
      if (spec.min > spec.max) throw new Error("最小值不能大于最大值");
      bits = Math.log2(Math.max(1, spec.max - spec.min + 1));
      for (let i = 0; i < count; i += 1) {
        items.push(String(spec.min === spec.max ? spec.min : spec.min + randInt(spec.max - spec.min + 1)));
      }
      break;
    case "pick": {
      const lines = spec.pool
        .split("\n")
        .map((l) => l.replace(/\s+$/, ""))
        .filter((l) => l.trim().length > 0);
      if (!lines.length) throw new Error("候选清单为空");
      for (let i = lines.length - 1; i > 0; i -= 1) {
        const j = randInt(i + 1);
        [lines[i], lines[j]] = [lines[j], lines[i]];
      }
      items.push(...lines.slice(0, Math.min(count, lines.length)));
      break;
    }
    default:
      throw new Error(`不认识的类型：${spec.kind}`);
  }
  return {
    items,
    output: items.join("\n"),
    count: items.length,
    bits,
    alphabetSize,
    elapsedUs: Math.round((performance.now() - started) * 1000),
  };
}

export async function devCall<T>(cmd: string, args: Record<string, unknown>): Promise<T> {
  const started = performance.now();
  const done = (v: unknown): T => v as T;

  switch (cmd) {
    case "json_format": {
      const input = String(args.input ?? "");
      const indent = Number(args.indent ?? 2);
      const sortKeys = Boolean(args.sortKeys);
      const minify = Boolean(args.minify);
      const bytesIn = te.encode(input).length;
      if (!input.trim()) {
        return done({
          ok: false,
          output: "",
          error: { message: "输入为空", line: 1, column: 1, snippet: "" },
          stats: { bytesIn, bytesOut: 0, nodes: 0, depth: 0, objects: 0, arrays: 0 },
          elapsedUs: Math.round((performance.now() - started) * 1000),
        } as JsonOutcome) as T;
      }
      try {
        let value: unknown = JSON.parse(input);
        if (sortKeys) value = sortValue(value);
        const output = minify ? JSON.stringify(value) : JSON.stringify(value, null, indent);
        const s = shape(value);
        return done({
          ok: true,
          output,
          error: null,
          stats: { bytesIn, bytesOut: te.encode(output).length, ...s },
          elapsedUs: Math.round((performance.now() - started) * 1000),
        } as JsonOutcome) as T;
      } catch (e) {
        const loc = locate(input, e);
        return done({
          ok: false,
          output: "",
          error: { message: String(e), ...loc },
          stats: { bytesIn, bytesOut: 0, nodes: 0, depth: 0, objects: 0, arrays: 0 },
          elapsedUs: Math.round((performance.now() - started) * 1000),
        } as JsonOutcome) as T;
      }
    }

    case "digest": {
      const algorithm = String(args.algorithm ?? "sha256").toLowerCase();
      const input = String(args.input ?? "");
      const key = args.key == null ? "" : String(args.key);
      const data = te.encode(input);
      let raw: Uint8Array;
      if (algorithm === "hmac-sha256") {
        const k = await crypto.subtle.importKey(
          "raw",
          te.encode(key),
          { name: "HMAC", hash: "SHA-256" },
          false,
          ["sign"],
        );
        raw = new Uint8Array(await crypto.subtle.sign("HMAC", k, data as unknown as BufferSource));
      } else {
        const map: Record<string, string> = { sha1: "SHA-1", sha256: "SHA-256", sha512: "SHA-512" };
        const name = map[algorithm];
        if (!name) throw new Error(`浏览器预览模式不支持 ${algorithm}，请用桌面端`);
        raw = new Uint8Array(await crypto.subtle.digest(name, data as unknown as BufferSource));
      }
      return done({
        algorithm,
        hex: hex(raw),
        base64: toB64(raw),
        bits: raw.length * 8,
        bytes: raw.length,
      } as HashOutcome) as T;
    }

    case "aes_encrypt": {
      const spec = args.spec as AesSpec;
      const plaintext = String(args.plaintext ?? "");
      if (spec.keyMode !== "password") throw new Error("浏览器预览只实现了「口令 + GCM」，其它组合请在桌面端运行");
      if (spec.mode !== "gcm") throw new Error("浏览器预览只实现了 GCM，CBC / CTR / ECB 请在桌面端运行");
      if (!spec.secret) throw new Error("请先设置一个口令");
      const salt = crypto.getRandomValues(new Uint8Array(16));
      const nonce =
        spec.ivMode === "custom"
          ? decodeByEncoding(spec.iv, spec.ivEncoding)
          : crypto.getRandomValues(new Uint8Array(12));
      if (nonce.length !== 12 && nonce.length !== 16) {
        throw new Error(`GCM 的 IV 需要 12 或 16 字节，当前 ${nonce.length} 字节`);
      }
      const key = await deriveAes(spec.secret, salt);
      const ct = new Uint8Array(
        await crypto.subtle.encrypt(
          { name: "AES-GCM", iv: nonce as unknown as BufferSource },
          key,
          te.encode(plaintext) as unknown as BufferSource,
        ),
      );
      return done({
        output: `ikt2.gcm.none.${toB64(salt)}.${toB64(nonce)}.${toB64(ct)}`,
        algorithm: "AES-256-GCM",
        keyBits: 256,
        ivHex: hex(nonce),
        ivEmbedded: true,
        padding: "none",
        notes: [],
      } as AesOutcome) as T;
    }

    case "aes_decrypt": {
      const spec = args.spec as AesSpec;
      const payload = String(args.payload ?? "").trim();
      if (!spec.secret) throw new Error("请填写口令");
      const seg = payload.split(".");
      let salt: Uint8Array;
      let nonce: Uint8Array;
      let ct: Uint8Array;
      let notes: string[] = [];
      if (seg[0] === "ikt2") {
        if (seg.length !== 6) throw new Error("ikt2 密文段数不对");
        if (seg[1] !== "gcm") throw new Error(`浏览器预览无法解密 ${seg[1].toUpperCase()}，请在桌面端打开`);
        salt = fromB64(seg[3]);
        nonce = fromB64(seg[4]);
        ct = fromB64(seg[5]);
      } else if (seg[0] === "ikt1") {
        if (seg.length !== 4) throw new Error("ikt1 密文段数不对");
        salt = fromB64(seg[1]);
        nonce = fromB64(seg[2]);
        ct = fromB64(seg[3]);
        notes = ["按旧版 ikt1 封装解密"];
      } else {
        throw new Error("浏览器预览只支持 ikt1 / ikt2-GCM 封装密文，裸密文请用桌面端");
      }
      const key = await deriveAes(spec.secret, salt);
      try {
        const plain = await crypto.subtle.decrypt(
          { name: "AES-GCM", iv: nonce as unknown as BufferSource },
          key,
          ct as unknown as BufferSource,
        );
        return done({
          output: td.decode(plain),
          algorithm: "AES-256-GCM",
          keyBits: 256,
          ivHex: hex(nonce),
          ivEmbedded: true,
          padding: "none",
          notes,
        } as AesOutcome) as T;
      } catch {
        throw new Error("解密失败：口令不正确，或密文已损坏");
      }
    }

    case "base64_encode": {
      const input = String(args.input ?? "");
      return toB64(te.encode(input), Boolean(args.urlSafe), Boolean(args.noPad)) as unknown as T;
    }

    case "base64_decode": {
      const raw = fromB64(String(args.input ?? ""));
      return td.decode(raw) as unknown as T;
    }

    case "url_encode": {
      const input = String(args.input ?? "");
      const spec = (args.spec ?? {}) as Partial<UrlSpec>;
      const keepUri = spec.keep === "uri";
      const plus = Boolean(spec.plusForSpace);
      const upper = spec.upperHex !== false;
      const URI_DELIM = ";/?:@&=+$,#";
      let out = "";
      let escapes = 0;
      for (const b of te.encode(input)) {
        const unreserved =
          (b >= 0x30 && b <= 0x39) ||
          (b >= 0x41 && b <= 0x5a) ||
          (b >= 0x61 && b <= 0x7a) ||
          b === 0x2d ||
          b === 0x5f ||
          b === 0x2e ||
          b === 0x7e ||
          b === 0x21 ||
          b === 0x27 ||
          b === 0x28 ||
          b === 0x29 ||
          b === 0x2a;
        if (unreserved || (keepUri && URI_DELIM.includes(String.fromCharCode(b)))) {
          out += String.fromCharCode(b);
        } else if (b === 0x20 && plus) {
          out += "+";
        } else {
          const h = b.toString(16).padStart(2, "0");
          out += `%${upper ? h.toUpperCase() : h}`;
          escapes += 1;
        }
      }
      return done({ output: out, outputBytes: out.length, escapes } as UrlOutcome) as T;
    }

    case "url_decode": {
      const input = String(args.input ?? "");
      const spec = (args.spec ?? {}) as Partial<UrlSpec>;
      const plus = Boolean(spec.plusForSpace);
      const src = te.encode(input);
      const out: number[] = [];
      let escapes = 0;
      const charPos = (byteIdx: number) => td.decode(src.slice(0, byteIdx)).length + 1;
      for (let i = 0; i < src.length; i += 1) {
        const b = src[i];
        if (b === 0x25) {
          if (i + 2 >= src.length) throw new Error(`第 ${charPos(i)} 个字符处的 % 转义不完整`);
          const h = td.decode(src.slice(i + 1, i + 3));
          if (!/^[0-9a-fA-F]{2}$/.test(h)) {
            throw new Error(`第 ${charPos(i + 1)} 个字符处 % 后面不是两位十六进制`);
          }
          out.push(parseInt(h, 16));
          escapes += 1;
          i += 2;
        } else if (b === 0x2b && plus) {
          out.push(0x20);
        } else {
          out.push(b);
        }
      }
      const text = td.decode(new Uint8Array(out));
      return done({
        output: text,
        outputBytes: te.encode(text).length,
        escapes,
      } as UrlOutcome) as T;
    }

    case "ascii_encode": {
      const input = String(args.input ?? "");
      const spec = (args.spec ?? {}) as Partial<AsciiSpec>;
      const style = spec.style ?? "js";
      const upper = Boolean(spec.upperHex);
      const pad = (n: number, width: number) => {
        const s = n.toString(16).padStart(width, "0");
        return upper ? s.toUpperCase() : s;
      };
      const width = (cp: number) => (cp <= 0xffff ? 4 : cp <= 0xfffff ? 5 : 6);
      let out = "";
      let converted = 0;
      let astral = 0;
      for (const ch of input) {
        const cp = ch.codePointAt(0) as number;
        if (cp < 0x80) {
          out += ch;
          continue;
        }
        converted += 1;
        if (cp > 0xffff) astral += 1;
        if (style === "htmlHex") {
          out += `&#x${pad(cp, width(cp))};`;
        } else if (style === "codePoint") {
          out += `U+${pad(cp, width(cp))}`;
        } else if (cp <= 0xffff) {
          out += `\\u${pad(cp, 4)}`;
        } else {
          const v = cp - 0x10000;
          out += `\\u${pad(0xd800 + (v >> 10), 4)}\\u${pad(0xdc00 + (v & 0x3ff), 4)}`;
        }
      }
      return done({ output: out, outputBytes: out.length, converted, astral } as AsciiOutcome) as T;
    }

    case "ascii_decode": {
      const chars = [...String(args.input ?? "")];
      const scan = (start: number, max: number, radix: number): [number, number] => {
        let val = 0;
        let n = 0;
        while (n < max && start + n < chars.length) {
          const c = chars[start + n];
          const v = c.length === 1 ? parseInt(c, radix) : NaN;
          if (Number.isNaN(v)) break;
          val = val * radix + v;
          n += 1;
        }
        return [val, n];
      };
      let out = "";
      let converted = 0;
      let astral = 0;
      let i = 0;
      const emit = (cp: number, pos: number) => {
        if (cp > 0x10ffff || (cp >= 0xd800 && cp <= 0xdfff)) {
          throw new Error(`第 ${pos + 1} 个字符处的码点不是合法字符（超出 Unicode 范围或为孤立代理项）`);
        }
        out += String.fromCodePoint(cp);
        if (cp > 0xffff) astral += 1;
        converted += 1;
      };
      while (i < chars.length) {
        const c = chars[i];

        if (c === "\\" && (chars[i + 1] === "u" || chars[i + 1] === "U")) {
          const braced = chars[i + 2] === "{";
          let cp = 0;
          let consumed = 0;
          if (braced) {
            const [val, n] = scan(i + 3, 6, 16);
            if (n > 0 && chars[i + 3 + n] === "}") {
              cp = val;
              consumed = 3 + n + 1;
            }
          } else {
            const [val, n] = scan(i + 2, 4, 16);
            if (n === 4) {
              cp = val;
              consumed = 6;
            }
          }
          if (consumed > 0) {
            let total = consumed;
            let finalCp = cp;
            if (cp >= 0xd800 && cp <= 0xdbff) {
              const next = i + consumed;
              let lo = 0;
              let ln = 0;
              if (chars[next] === "\\" && (chars[next + 1] === "u" || chars[next + 1] === "U")) {
                [lo, ln] = scan(next + 2, 4, 16);
              }
              if (ln !== 4 || lo < 0xdc00 || lo > 0xdfff) {
                throw new Error(`第 ${i + 1} 个字符处是孤立的高代理项，后面缺少配对的 \\uDC00-\\uDFFF`);
              }
              finalCp = 0x10000 + ((cp - 0xd800) << 10) + (lo - 0xdc00);
              total += 6;
            } else if (cp >= 0xdc00 && cp <= 0xdfff) {
              throw new Error(`第 ${i + 1} 个字符处是孤立的低代理项，前面缺少配对的高代理项`);
            }
            emit(finalCp, i);
            i += total;
            continue;
          }
        }

        if (c === "&" && chars[i + 1] === "#") {
          const hex = chars[i + 2] === "x" || chars[i + 2] === "X";
          const start = hex ? i + 3 : i + 2;
          const [val, n] = scan(start, hex ? 6 : 8, hex ? 16 : 10);
          if (n > 0 && chars[start + n] === ";") {
            emit(val, i);
            i = start + n + 1;
            continue;
          }
        }

        if ((c === "U" || c === "u") && chars[i + 1] === "+") {
          const [val, n] = scan(i + 2, 6, 16);
          if (n >= 4) {
            emit(val, i);
            i += 2 + n;
            continue;
          }
        }

        out += c;
        i += 1;
      }
      return done({
        output: out,
        outputBytes: te.encode(out).length,
        converted,
        astral,
      } as AsciiOutcome) as T;
    }

    case "regex_test": {
      const started = performance.now();
      const spec = (args.spec ?? {}) as Partial<RegexSpec>;
      const pattern = String(args.pattern ?? "");
      const text = String(args.text ?? "");
      const { re, names } = buildJsRegex(pattern, spec);
      const starts = lineStartOffsets(text);
      const MAX = 2000;
      const matches: RegexMatch[] = [];
      let count = 0;
      let m: RegExpExecArray | null;
      while ((m = re.exec(text)) !== null) {
        count += 1;
        if (matches.length < MAX) {
          const [line, column] = locateLine(starts, m.index);
          const groups: RegexGroup[] = [];
          for (let gi = 1; gi < m.length; gi += 1) {
            groups.push({ index: gi, name: names[gi - 1] ?? null, text: m[gi] ?? null });
          }
          matches.push({
            index: matches.length + 1,
            start: m.index,
            end: m.index + m[0].length,
            line,
            column,
            text: m[0],
            groups,
          });
        }
        if (m[0] === "") re.lastIndex += 1;
      }
      return done({
        matches,
        count,
        truncated: count > matches.length,
        groupNames: names.filter((n): n is string => !!n),
        elapsedUs: Math.round((performance.now() - started) * 1000),
      } as RegexOutcome) as T;
    }

    case "regex_replace": {
      const started = performance.now();
      const spec = (args.spec ?? {}) as Partial<RegexSpec>;
      const pattern = String(args.pattern ?? "");
      const text = String(args.text ?? "");
      const tpl = String(args.replacement ?? "");
      const { re } = buildJsRegex(pattern, spec);
      let out = "";
      let last = 0;
      let replacements = 0;
      let m: RegExpExecArray | null;
      while ((m = re.exec(text)) !== null) {
        out += text.slice(last, m.index) + expandTemplate(tpl, m);
        last = m.index + m[0].length;
        replacements += 1;
        if (m[0] === "") re.lastIndex += 1;
      }
      out += text.slice(last);
      return done({
        output: out,
        outputBytes: te.encode(out).length,
        replacements,
        changed: replacements > 0 && out !== text,
        elapsedUs: Math.round((performance.now() - started) * 1000),
      } as ReplaceOutcome) as T;
    }

    case "xml_json_to_xml": {
      const started = performance.now();
      const spec = (args.spec ?? {}) as XmlSpec;
      const out = xmlJsonToXml(spec, String(args.input ?? ""));
      out.elapsedUs = Math.round((performance.now() - started) * 1000);
      return done(out) as T;
    }

    case "xml_xml_to_json": {
      const started = performance.now();
      const spec = (args.spec ?? {}) as XmlSpec;
      const { doc } = xmlParse(String(args.input ?? ""));
      const obj: Record<string, unknown> = {};
      for (const node of Array.from(doc.childNodes)) {
        if (node.nodeType !== 1) continue;
        const el = node as Element;
        const value = xmlElementValue(el, spec);
        const existing = obj[el.tagName];
        if (existing === undefined) obj[el.tagName] = value;
        else if (Array.isArray(existing)) existing.push(value);
        else obj[el.tagName] = [existing, value];
      }
      if (Object.keys(obj).length === 0) throw new Error("输入里没有元素");
      const output = spec.indent === 0 ? JSON.stringify(obj) : JSON.stringify(obj, null, spec.indent);
      // 统计口径和其他模式一致，只是这里不需要序列化结果
      const { stats } = xmlSerializeDoc(doc, { ...spec, indent: 0, declaration: false }, null);
      return done({
        output,
        outputBytes: te.encode(output).length,
        root: doc.documentElement?.tagName ?? null,
        elements: stats.elements,
        attributes: stats.attributes,
        depth: stats.depth,
        comments: stats.comments,
        renamed: 0,
        elapsedUs: Math.round((performance.now() - started) * 1000),
      } as XmlOutcome) as T;
    }

    case "xml_format": {
      const started = performance.now();
      const spec = (args.spec ?? {}) as XmlSpec;
      const { doc, decl } = xmlParse(String(args.input ?? ""));
      return done(xmlToXmlOutcome(doc, decl, spec, started)) as T;
    }

    case "xml_minify": {
      const started = performance.now();
      const spec = (args.spec ?? {}) as XmlSpec;
      const { doc, decl } = xmlParse(String(args.input ?? ""));
      return done(xmlToXmlOutcome(doc, decl, { ...spec, indent: 0 }, started)) as T;
    }

    case "jwt_decode": {      const token = String(args.token ?? "").trim().replace(/^Bearer\s+/i, "");
      const seg = token.split(".");
      if (seg.length < 2) throw new Error("JWT 至少需要 header.payload 两段");
      const read = (s: string) => {
        const text = td.decode(fromB64(s));
        try {
          return { text: JSON.stringify(JSON.parse(text), null, 2), json: JSON.parse(text) };
        } catch {
          return { text, json: null };
        }
      };
      const head = read(seg[0]);
      const body = read(seg[1]);
      const now = Math.floor(Date.now() / 1000);
      const num = (k: string) => (body.json && typeof body.json[k] === "number" ? body.json[k] : null);
      const iso = (sec: number | null) =>
        sec == null ? null : new Date(sec * 1000).toLocaleString("zh-CN", { hour12: false });
      const exp = num("exp");
      return done({
        algorithm: head.json?.alg ?? "unknown",
        typ: head.json?.typ ?? "JWT",
        header: head.text,
        payload: body.text,
        signature: seg[2] ?? "",
        iat: num("iat"),
        exp,
        nbf: num("nbf"),
        iatText: iso(num("iat")),
        expText: iso(exp),
        nbfText: iso(num("nbf")),
        expired: exp == null ? null : exp < now,
        secondsLeft: exp == null ? null : exp - now,
        missingSignature: !seg[2],
      } as JwtParts) as T;
    }

    case "rsa_generate_keypair": {
      const bits = Number(args.bits ?? 2048);
      if (![2048, 3072, 4096].includes(bits)) throw new Error("只支持 2048 / 3072 / 4096 位");
      const started = performance.now();
      const pair = (await crypto.subtle.generateKey(
        {
          name: "RSA-OAEP",
          modulusLength: bits,
          publicExponent: new Uint8Array([1, 0, 1]),
          hash: "SHA-256",
        },
        true,
        ["encrypt", "decrypt"],
      )) as CryptoKeyPair;
      const spki = new Uint8Array(await crypto.subtle.exportKey("spki", pair.publicKey));
      const pkcs8 = new Uint8Array(await crypto.subtle.exportKey("pkcs8", pair.privateKey));
      const fpr = new Uint8Array(
        await crypto.subtle.digest("SHA-256", spki as unknown as BufferSource),
      );
      return done({
        publicPem: pemOf(spki, "PUBLIC KEY"),
        privatePem: pemOf(pkcs8, "PRIVATE KEY"),
        publicPkcs1Pem: "",
        privatePkcs1Pem: "",
        bits,
        exponent: "65537",
        fingerprintSha256: hex(fpr),
        fingerprintOpenssh: `SHA256:${toB64(fpr, true, true)}`,
        elapsedMs: Math.round(performance.now() - started),
      } as RsaKeyPair) as T;
    }

    case "rsa_inspect": {
      const pem = String(args.key ?? "");
      if (!pem.trim()) throw new Error("先粘贴一段 PEM");
      if (/BEGIN RSA (PUBLIC|PRIVATE) KEY/.test(pem)) {
        throw new Error("浏览器预览不支持 PKCS#1 格式的密钥，请在桌面端运行");
      }
      const isPrivate = /PRIVATE KEY/.test(pem);
      const key = await crypto.subtle.importKey(
        isPrivate ? "pkcs8" : "spki",
        pemBody(pem) as unknown as BufferSource,
        { name: "RSA-OAEP", hash: "SHA-256" },
        true,
        [isPrivate ? "decrypt" : "encrypt"],
      );
      const info: RsaKeyInfo = {
        kind: isPrivate ? "private" : "public",
        bits: rsaBits(key),
        exponent: "65537",
        fingerprintSha256: "",
        fingerprintOpenssh: "",
      };
      if (!isPrivate) {
        const spki = new Uint8Array(await crypto.subtle.exportKey("spki", key));
        const digest = new Uint8Array(
          await crypto.subtle.digest("SHA-256", spki as unknown as BufferSource),
        );
        info.fingerprintSha256 = hex(digest);
        info.fingerprintOpenssh = `SHA256:${toB64(digest, true, true)}`;
      }
      return done(info) as T;
    }

    case "rsa_encrypt": {
      const spec = args.spec as RsaSpec;
      const pem = String(args.publicKey ?? "");
      const plaintext = String(args.plaintext ?? "");
      if (spec.scheme === "pkcs1v15") {
        throw new Error("浏览器预览不支持 PKCS#1 v1.5，请在桌面端运行");
      }
      const hash = spec.scheme === "oaep-sha1" ? "SHA-1" : "SHA-256";
      const key = await importRsa(pem, "encrypt", hash);
      const keyBits = rsaBits(key);
      const notes: string[] = [];
      let output = "";
      try {
        if (spec.scheme === "hybrid") {
          const aesKey = crypto.getRandomValues(new Uint8Array(32));
          const iv = crypto.getRandomValues(new Uint8Array(12));
          const wrapped = new Uint8Array(
            await crypto.subtle.encrypt(
              { name: "RSA-OAEP" },
              key,
              aesKey as unknown as BufferSource,
            ),
          );
          const ck = await crypto.subtle.importKey("raw", aesKey as unknown as BufferSource, "AES-GCM", false, [
            "encrypt",
          ]);
          const body = new Uint8Array(
            await crypto.subtle.encrypt(
              { name: "AES-GCM", iv: iv as unknown as BufferSource },
              ck,
              te.encode(plaintext) as unknown as BufferSource,
            ),
          );
          output = `ikt3.${toB64(wrapped)}.${toB64(iv)}.${toB64(body)}`;
          notes.push("浏览器预览同样支持 ikt3 混合封装");
        } else {
          const ct = new Uint8Array(
            await crypto.subtle.encrypt(
              { name: "RSA-OAEP" },
              key,
              te.encode(plaintext) as unknown as BufferSource,
            ),
          );
          output = spec.outputEncoding === "hex" ? hex(ct) : toB64(ct);
        }
      } catch {
        throw new Error("加密失败：明文超过该密钥能承载的长度，或密钥类型不匹配");
      }
      const k = keyBits / 8;
      const maxBytes =
        spec.scheme === "hybrid" ? 0 : spec.scheme === "oaep-sha256" ? k - 66 : k - 42;
      return done({
        output,
        scheme:
          spec.scheme === "hybrid"
            ? "RSA-OAEP + AES-256-GCM"
            : spec.scheme === "oaep-sha1"
              ? "RSA-OAEP-SHA1"
              : "RSA-OAEP-SHA256",
        keyBits,
        fingerprintOpenssh: await rsaFingerprint(key),
        maxBytes,
        payloadBytes: te.encode(plaintext).length,
        elapsedMs: 0,
        notes,
      } as RsaOutcome) as T;
    }

    case "rsa_decrypt": {
      const spec = args.spec as RsaSpec;
      const pem = String(args.privateKey ?? "");
      const payload = String(args.payload ?? "").trim();
      const isHybrid = payload.startsWith("ikt3.");
      if (!isHybrid && spec.scheme === "pkcs1v15") {
        throw new Error("浏览器预览不支持 PKCS#1 v1.5，请在桌面端运行");
      }
      const hash = !isHybrid && spec.scheme === "oaep-sha1" ? "SHA-1" : "SHA-256";
      const key = await importRsa(pem, "decrypt", hash);
      let plain: Uint8Array;
      if (isHybrid) {
        const seg = payload.split(".");
        if (seg.length !== 4) throw new Error("ikt3 密文段数不对");
        const wrapped = fromB64(seg[1]);
        const iv = fromB64(seg[2]);
        const body = fromB64(seg[3]);
        const aesKey = new Uint8Array(
          await crypto.subtle.decrypt(
            { name: "RSA-OAEP" },
            key,
            wrapped as unknown as BufferSource,
          ),
        );
        const ck = await crypto.subtle.importKey("raw", aesKey as unknown as BufferSource, "AES-GCM", false, [
          "decrypt",
        ]);
        try {
          plain = new Uint8Array(
            await crypto.subtle.decrypt(
              { name: "AES-GCM", iv: iv as unknown as BufferSource },
              ck,
              body as unknown as BufferSource,
            ),
          );
        } catch {
          throw new Error("AES-GCM 认证失败：密文被改动过");
        }
      } else {
        const cipher =
          spec.inputEncoding === "hex"
            ? hexBytes(payload)
            : spec.inputEncoding === "base64"
              ? fromB64(payload)
              : /^[0-9a-f]+$/i.test(payload) && payload.length % 2 === 0
                ? hexBytes(payload)
                : fromB64(payload);
        try {
          plain = new Uint8Array(
            await crypto.subtle.decrypt(
              { name: "RSA-OAEP" },
              key,
              cipher as unknown as BufferSource,
            ),
          );
        } catch {
          throw new Error("解密失败：私钥与密文不匹配，或方案选错了");
        }
      }
      return done({
        output: td.decode(plain),
        scheme: isHybrid
          ? "RSA-OAEP + AES-256-GCM"
          : hash === "SHA-1"
            ? "RSA-OAEP-SHA1"
            : "RSA-OAEP-SHA256",
        keyBits: rsaBits(key),
        fingerprintOpenssh: "",
        maxBytes: 0,
        payloadBytes: plain.length,
        elapsedMs: 0,
        notes: [],
      } as RsaOutcome) as T;
    }

    case "time_convert": {
      return done(timeConvert(args.spec as TimeSpec, String(args.input ?? ""), started)) as T;
    }

    case "time_now": {
      const offset = tzOffsetSeconds((args.spec as TimeSpec).tz);
      return done(buildTime(Date.now(), offset, "toDate", "s", started)) as T;
    }

    case "random_generate": {
      return done(randomGenerate(args.spec as RandomSpec, started)) as T;
    }

    case "net_ping":
    case "net_probe":
    case "net_dns":
    case "net_geo":
      throw new Error("浏览器预览没有 socket 与归属地库，请在桌面端运行");

    case "http_build":
    case "http_send":
      throw new Error("浏览器预览没有 HTTP 客户端，请在桌面端运行");

    default:
      throw new Error(`未知命令：${cmd}`);
  }
}
