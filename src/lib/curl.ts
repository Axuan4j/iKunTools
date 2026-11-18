import type { BuiltRequest, HttpOptions } from "./api";

/**
 * 把已经组装好的请求翻译成 cURL 命令。
 * 输入是 Rust 侧 `build` 的产物，所以命令里的 URL、头、体与实际发出的完全一致。
 */
export type CurlStyle = "bash" | "bashInline" | "powershell";

const SAFE = /^[A-Za-z0-9_@%+=:,./-]+$/;

/** POSIX shell：单引号包住，内部的 ' 用 '\'' 收口 */
function bashQuote(s: string): string {
  if (s === "") return "''";
  if (SAFE.test(s)) return s;
  return `'${s.replace(/'/g, `'\\''`)}'`;
}

/** PowerShell：单引号包住，内部的 ' 双写；反引号、$ 在单引号里都是字面量 */
function psQuote(s: string): string {
  if (s === "") return "''";
  if (SAFE.test(s)) return s;
  return `'${s.replace(/'/g, "''")}'`;
}

export function toCurl(req: BuiltRequest, opts: HttpOptions, style: CurlStyle = "bash"): string {
  const ps = style === "powershell";
  const q = ps ? psQuote : bashQuote;
  const cmd = ps ? "curl.exe" : "curl";
  const parts: string[] = [];

  // -I 已经隐含 HEAD，再给 -X HEAD 会让 curl 卡在等响应体
  const headOnly = req.method === "HEAD";
  if (headOnly) {
    parts.push("-I");
  } else if (req.method !== "GET") {
    parts.push(`-X ${req.method}`);
  }

  parts.push(q(req.url));

  for (const h of req.headers) {
    // multipart 的 boundary 由 curl 自己生成，不能再带 Content-Type
    if (req.multipart && h.name.toLowerCase() === "content-type") continue;
    parts.push(`-H ${q(`${h.name}: ${h.value}`)}`);
  }

  if (!headOnly) {
    if (req.multipart) {
      for (const f of req.formFields) parts.push(`-F ${q(`${f.name}=${f.value}`)}`);
    } else if (req.hasBody && req.body.length > 0) {
      parts.push(`--data-raw ${q(req.body)}`);
    }
  }

  if (req.proxy) parts.push(`-x ${q(req.proxy)}`);
  if (!opts.verifyTls) parts.push("-k");
  if (opts.followRedirects) parts.push("-L");
  if (opts.timeoutMs > 0) parts.push(`--max-time ${Math.max(1, Math.round(opts.timeoutMs / 1000))}`);

  if (style === "bashInline") return `${cmd} ${parts.join(" ")}`;

  const cont = ps ? "`" : "\\";
  const lines = parts.map((p, i) => (i === parts.length - 1 ? `  ${p}` : `  ${p} ${cont}`));
  return [cmd, ...lines].join("\n");
}
