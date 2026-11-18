import { invoke } from "@tauri-apps/api/core";
import { devCall } from "./devFallback";

export type JsonStats = {
  bytesIn: number;
  bytesOut: number;
  nodes: number;
  depth: number;
  objects: number;
  arrays: number;
};

export type JsonError = {
  message: string;
  line: number;
  column: number;
  snippet: string;
};

export type JsonOutcome = {
  ok: boolean;
  output: string;
  error: JsonError | null;
  stats: JsonStats;
  elapsedUs: number;
};

export type HashOutcome = {
  algorithm: string;
  hex: string;
  base64: string;
  bits: number;
  bytes: number;
};

export type AesMode = "gcm" | "cbc" | "ctr" | "ecb";
export type AesPadding = "pkcs7" | "zero" | "none";
export type AesKeyMode = "password" | "raw";
export type AesIvMode = "random" | "custom";
export type AesEncoding = "utf8" | "hex" | "base64";
export type AesOutputEncoding = "base64" | "hex";

export type AesSpec = {
  mode: AesMode;
  keyMode: AesKeyMode;
  secret: string;
  keyEncoding: AesEncoding;
  ivMode: AesIvMode;
  iv: string;
  ivEncoding: AesEncoding;
  padding: AesPadding;
  outputEncoding: AesOutputEncoding;
};

export type AesOutcome = {
  output: string;
  algorithm: string;
  keyBits: number;
  ivHex: string;
  ivEmbedded: boolean;
  padding: string;
  notes: string[];
};

export type RsaKeyPair = {
  publicPem: string;
  privatePem: string;
  publicPkcs1Pem: string;
  privatePkcs1Pem: string;
  bits: number;
  exponent: string;
  fingerprintSha256: string;
  fingerprintOpenssh: string;
  elapsedMs: number;
};

export type RsaKeyInfo = {
  kind: "public" | "private";
  bits: number;
  exponent: string;
  fingerprintSha256: string;
  fingerprintOpenssh: string;
};

export type RsaScheme = "oaep-sha256" | "oaep-sha1" | "pkcs1v15" | "hybrid";
export type RsaInputEncoding = "auto" | "base64" | "hex";
export type RsaOutputEncoding = "base64" | "hex";

export type RsaSpec = {
  scheme: RsaScheme;
  outputEncoding: RsaOutputEncoding;
  inputEncoding: RsaInputEncoding;
};

export type RsaOutcome = {
  output: string;
  scheme: string;
  keyBits: number;
  fingerprintOpenssh: string;
  maxBytes: number;
  payloadBytes: number;
  elapsedMs: number;
  notes: string[];
};

export type UrlKeep = "component" | "uri";

export type UrlSpec = {
  keep: UrlKeep;
  plusForSpace: boolean;
  upperHex: boolean;
};

export type UrlOutcome = {
  output: string;
  outputBytes: number;
  escapes: number;
};

/** js: \uXXXX（JSON/JS 转义）· htmlHex: &#xXXXX; · codePoint: U+XXXX */
export type AsciiStyle = "js" | "htmlHex" | "codePoint";

export type AsciiSpec = {
  style: AsciiStyle;
  upperHex: boolean;
};

export type AsciiOutcome = {
  output: string;
  outputBytes: number;
  converted: number;
  astral: number;
};

export type RegexSpec = {
  ignoreCase: boolean;
  multiLine: boolean;
  dotAll: boolean;
  extended: boolean;
  unicode: boolean;
};

export type RegexGroup = {
  index: number;
  name: string | null;
  text: string | null;
};

export type RegexMatch = {
  index: number;
  start: number;
  end: number;
  line: number;
  column: number;
  text: string;
  groups: RegexGroup[];
};

export type RegexOutcome = {
  matches: RegexMatch[];
  count: number;
  truncated: boolean;
  groupNames: string[];
  elapsedUs: number;
};

export type ReplaceOutcome = {
  output: string;
  outputBytes: number;
  replacements: number;
  changed: boolean;
  elapsedUs: number;
};

/** 属性前缀默认 @，文本键默认 #text，和 fast-xml-parser 保持一致 */
export type XmlSpec = {
  indent: number;
  declaration: boolean;
  root: string;
  attrPrefix: string;
  textKey: string;
  trim: boolean;
  numbers: boolean;
  keepComments: boolean;
};

export type XmlOutcome = {
  output: string;
  outputBytes: number;
  root: string | null;
  elements: number;
  attributes: number;
  depth: number;
  comments: number;
  renamed: number;
  elapsedUs: number;
};

export type TimeSpec = {
  /** `+08:00` / `Z` / `-05:00` 这类偏移写法 */
  tz: string;
  /** auto / s / ms / us / ns，仅在输入是纯数字时生效 */
  unit: string;
};

export type TimeOutcome = {
  direction: "toDate" | "toTimestamp";
  unit: string;
  seconds: number;
  millis: number;
  micros: number;
  nanos: number;
  utcIso: string;
  utcText: string;
  localIso: string;
  localText: string;
  tzLabel: string;
  weekday: string;
  dayOfYear: number;
  week: number;
  relative: string;
  nowMs: number;
  elapsedUs: number;
};

export type RandomKind =
  | "uuid4"
  | "uuid7"
  | "ulid"
  | "nanoid"
  | "password"
  | "text"
  | "bytes"
  | "int"
  | "pick";

export type RandomSpec = {
  kind: RandomKind;
  count: number;
  length: number;
  alphabet: string;
  digits: boolean;
  lower: boolean;
  upper: boolean;
  symbols: boolean;
  excludeAmbiguous: boolean;
  bytesLen: number;
  bytesEncoding: "hex" | "base64";
  min: number;
  max: number;
  pool: string;
};

export type RandomOutcome = {
  items: string[];
  output: string;
  count: number;
  bits: number;
  alphabetSize: number;
  elapsedUs: number;
};

export type PingSpec = {
  host: string;
  port: number;
  count: number;
  timeoutMs: number;
};

export type PingSample = {
  seq: number;
  ms: number | null;
  error: string | null;
};

export type PingOutcome = {
  host: string;
  port: number;
  addrs: string[];
  samples: PingSample[];
  sent: number;
  received: number;
  lossPct: number;
  minMs: number;
  avgMs: number;
  maxMs: number;
  elapsedUs: number;
};

export type ProbeSpec = {
  host: string;
  port: number;
  payload: string;
  appendNewline: boolean;
  timeoutMs: number;
  readMs: number;
};

export type ProbeOutcome = {
  host: string;
  port: number;
  addr: string;
  connectMs: number;
  sentBytes: number;
  recvBytes: number;
  text: string;
  hex: string;
  closed: boolean;
  elapsedUs: number;
};

export type AddrInfo = {
  ip: string;
  family: string;
};

export type DnsOutcome = {
  host: string;
  addrs: AddrInfo[];
  elapsedUs: number;
};

export type GeoOutcome = {
  ip: string;
  version: string;
  raw: string;
  country: string;
  region: string;
  province: string;
  city: string;
  isp: string;
  elapsedUs: number;
};

/** 请求头 / 查询参数 / 表单字段通用。enabled=false 的行不参与组装。 */
export type HttpKV = {
  name: string;
  value: string;
  enabled: boolean;
};

export type HttpBodyKind = "none" | "raw" | "urlencoded" | "form";
export type HttpAuthKind = "none" | "bearer" | "basic";

export type HttpOptions = {
  timeoutMs: number;
  followRedirects: boolean;
  maxRedirects: number;
  verifyTls: boolean;
  /** 最多读回多少字节的响应体 */
  maxBody: number;
  /** 走 HTTP CONNECT 代理 */
  proxyEnabled: boolean;
  /** `http://127.0.0.1:8080`、`127.0.0.1:8080` 都可 */
  proxyUrl: string;
  proxyUser: string;
  proxyPass: string;
};

export type HttpSpec = {
  method: string;
  url: string;
  params: HttpKV[];
  headers: HttpKV[];
  bodyKind: HttpBodyKind;
  body: string;
  bodyParams: HttpKV[];
  contentType: string;
  authKind: HttpAuthKind;
  authToken: string;
  authUser: string;
  authPass: string;
  options: HttpOptions;
};

/** 组装结果，和真正发出去的报文一致，cURL 命令据此生成 */
export type BuiltRequest = {
  method: string;
  url: string;
  headers: HttpKV[];
  body: string;
  bodyBytes: number;
  contentType: string | null;
  multipart: boolean;
  formFields: HttpKV[];
  /** 归一化后的代理地址（含凭据），cURL 的 -x 用它 */
  proxy: string | null;
  host: string;
  hasBody: boolean;
};

export type HttpOutcome = {
  status: number;
  statusText: string;
  httpVersion: string;
  headers: HttpKV[];
  /** 文本响应是原文；二进制响应是 Base64 */
  body: string;
  isBase64: boolean;
  bodyBytes: number;
  declaredBytes: number | null;
  truncated: boolean;
  elapsedMs: number;
  url: string;
  contentType: string;
};

export type JwtParts = {
  algorithm: string;
  typ: string;
  header: string;
  payload: string;
  signature: string;
  iat: number | null;
  exp: number | null;
  nbf: number | null;
  iatText: string | null;
  expText: string | null;
  nbfText: string | null;
  expired: boolean | null;
  secondsLeft: number | null;
  missingSignature: boolean;
};

/**
 * 唯一的前后端契约边界。
 * 正式链路一律走 Rust；浏览器里跑（`pnpm dev` 直接开页面）时才落到 WebCrypto 兜底实现。
 */
export const inTauri = (): boolean =>
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

function call<T>(cmd: string, args: Record<string, unknown>): Promise<T> {
  if (!inTauri()) return devCall<T>(cmd, args);
  return invoke<T>(cmd, args);
}

export const api = {
  jsonFormat: (input: string, indent: number, sortKeys: boolean, minify: boolean) =>
    call<JsonOutcome>("json_format", { input, indent, sortKeys, minify }),

  digest: (algorithm: string, input: string, key?: string) =>
    call<HashOutcome>("digest", { algorithm, input, key: key ?? null }),

  aesEncrypt: (spec: AesSpec, plaintext: string) =>
    call<AesOutcome>("aes_encrypt", { spec, plaintext }),

  aesDecrypt: (spec: AesSpec, payload: string) =>
    call<AesOutcome>("aes_decrypt", { spec, payload }),

  base64Encode: (input: string, urlSafe: boolean, noPad: boolean) =>
    call<string>("base64_encode", { input, urlSafe, noPad }),

  base64Decode: (input: string) => call<string>("base64_decode", { input }),

  urlEncode: (spec: UrlSpec, input: string) => call<UrlOutcome>("url_encode", { spec, input }),

  urlDecode: (spec: UrlSpec, input: string) => call<UrlOutcome>("url_decode", { spec, input }),

  asciiEncode: (spec: AsciiSpec, input: string) =>
    call<AsciiOutcome>("ascii_encode", { spec, input }),

  asciiDecode: (input: string) => call<AsciiOutcome>("ascii_decode", { input }),

  regexTest: (spec: RegexSpec, pattern: string, text: string) =>
    call<RegexOutcome>("regex_test", { spec, pattern, text }),

  regexReplace: (spec: RegexSpec, pattern: string, text: string, replacement: string) =>
    call<ReplaceOutcome>("regex_replace", { spec, pattern, text, replacement }),

  xmlJsonToXml: (spec: XmlSpec, input: string) =>
    call<XmlOutcome>("xml_json_to_xml", { spec, input }),

  xmlXmlToJson: (spec: XmlSpec, input: string) =>
    call<XmlOutcome>("xml_xml_to_json", { spec, input }),

  xmlFormat: (spec: XmlSpec, input: string) => call<XmlOutcome>("xml_format", { spec, input }),

  xmlMinify: (spec: XmlSpec, input: string) => call<XmlOutcome>("xml_minify", { spec, input }),

  jwtDecode: (token: string) => call<JwtParts>("jwt_decode", { token }),

  rsaGenerateKeypair: (bits: number) => call<RsaKeyPair>("rsa_generate_keypair", { bits }),

  rsaInspect: (key: string) => call<RsaKeyInfo>("rsa_inspect", { key }),

  rsaEncrypt: (spec: RsaSpec, publicKey: string, plaintext: string) =>
    call<RsaOutcome>("rsa_encrypt", { spec, publicKey, plaintext }),

  rsaDecrypt: (spec: RsaSpec, privateKey: string, payload: string) =>
    call<RsaOutcome>("rsa_decrypt", { spec, privateKey, payload }),

  timeConvert: (spec: TimeSpec, input: string) =>
    call<TimeOutcome>("time_convert", { spec, input }),

  timeNow: (spec: TimeSpec) => call<TimeOutcome>("time_now", { spec }),

  randomGenerate: (spec: RandomSpec) => call<RandomOutcome>("random_generate", { spec }),

  netPing: (spec: PingSpec) => call<PingOutcome>("net_ping", { spec }),

  netProbe: (spec: ProbeSpec) => call<ProbeOutcome>("net_probe", { spec }),

  netDns: (host: string) => call<DnsOutcome>("net_dns", { host }),

  netGeo: (target: string) => call<GeoOutcome>("net_geo", { target }),

  httpBuild: (spec: HttpSpec) => call<BuiltRequest>("http_build", { spec }),

  httpSend: (spec: HttpSpec) => call<HttpOutcome>("http_send", { spec }),
};
