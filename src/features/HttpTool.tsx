import { useEffect, useMemo, useState } from "react";
import CodeEditor from "../components/CodeEditor";
import { IconChevron, IconClose, IconHistory, IconSend, IconTerminal } from "../components/icons";
import { Pane, PrimaryBtn, Segmented, Toggle, copyText } from "../components/ui";
import {
  api,
  type BuiltRequest,
  type HttpAuthKind,
  type HttpBodyKind,
  type HttpKV,
  type HttpOptions,
  type HttpOutcome,
  type HttpSpec,
} from "../lib/api";
import { toCurl, type CurlStyle } from "../lib/curl";

const METHODS = ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"];

const DEFAULTS: HttpOptions = {
  timeoutMs: 30_000,
  followRedirects: true,
  maxRedirects: 10,
  verifyTls: true,
  maxBody: 5 * 1024 * 1024,
  proxyEnabled: false,
  proxyUrl: "",
  proxyUser: "",
  proxyPass: "",
};

const CONTENT_TYPES = [
  "application/json",
  "application/xml",
  "text/plain",
  "text/html",
  "application/x-www-form-urlencoded",
  "application/graphql",
];

const HISTORY_KEY = "ikt.http.history";

type HistoryItem = { at: number; method: string; url: string; spec: HttpSpec };
type Tab = "params" | "headers" | "body" | "auth" | "options";
type RespTab = "body" | "headers" | "curl";

const emptyKV = (): HttpKV => ({ name: "", value: "", enabled: true });

function loadHistory(): HistoryItem[] {
  try {
    const raw = localStorage.getItem(HISTORY_KEY);
    const arr = raw ? JSON.parse(raw) : [];
    return Array.isArray(arr) ? arr.slice(0, 20) : [];
  } catch {
    return [];
  }
}

function bytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / 1024 / 1024).toFixed(2)} MB`;
}

function looksJson(contentType: string, body: string): boolean {
  if (contentType.includes("json")) return true;
  const head = body.trimStart();
  return head.startsWith("{") || head.startsWith("[");
}

function CheckBox({ checked, onChange }: { checked: boolean; onChange: (v: boolean) => void }) {
  return (
    <button
      onClick={() => onChange(!checked)}
      className={`flex h-[13px] w-[13px] items-center justify-center rounded-[4px] border transition-colors ${
        checked ? "border-accent bg-accent text-white" : "border-line-strong bg-panel"
      }`}
    >
      {checked ? (
        <svg viewBox="0 0 16 16" className="h-[9px] w-[9px]" fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round" strokeLinejoin="round">
          <path d="M3.4 8.4 6.6 11.4 12.6 4.8" />
        </svg>
      ) : null}
    </button>
  );
}

function KVEditor({
  rows,
  onChange,
  keyPlaceholder = "键",
  valuePlaceholder = "值",
  actions,
}: {
  rows: HttpKV[];
  onChange: (rows: HttpKV[]) => void;
  keyPlaceholder?: string;
  valuePlaceholder?: string;
  actions?: React.ReactNode;
}) {
  const patch = (i: number, p: Partial<HttpKV>) => {
    const next = rows.map((r, idx) => (idx === i ? { ...r, ...p } : r));
    const last = next[next.length - 1];
    if (last && (last.name || last.value)) next.push(emptyKV());
    onChange(next);
  };

  const remove = (i: number) => {
    const next = rows.filter((_, idx) => idx !== i);
    onChange(next.length ? next : [emptyKV()]);
  };

  return (
    <div className="flex h-full flex-col">
      {actions ? (
        <div className="flex h-[34px] shrink-0 items-center gap-2 border-b border-line px-3">{actions}</div>
      ) : null}
      <div className="min-h-0 flex-1 overflow-auto">
        {rows.map((row, i) => (
          <div
            key={i}
            className="group flex h-[34px] items-center gap-2.5 border-b border-line px-3 focus-within:bg-[#fcfbf9]"
          >
            <CheckBox checked={row.enabled} onChange={(v) => patch(i, { enabled: v })} />
            <input
              value={row.name}
              onChange={(e) => patch(i, { name: e.target.value })}
              spellCheck={false}
              placeholder={keyPlaceholder}
              className={`min-w-0 flex-1 bg-transparent font-mono text-[12px] outline-none placeholder:font-sans placeholder:text-faint ${
                row.enabled ? "text-ink" : "text-faint"
              }`}
            />
            <input
              value={row.value}
              onChange={(e) => patch(i, { value: e.target.value })}
              spellCheck={false}
              placeholder={valuePlaceholder}
              className={`min-w-0 flex-[1.4] bg-transparent font-mono text-[12px] outline-none placeholder:font-sans placeholder:text-faint ${
                row.enabled ? "text-ink" : "text-faint"
              }`}
            />
            <button
              onClick={() => remove(i)}
              className="flex h-[18px] w-[18px] items-center justify-center rounded-[4px] text-faint opacity-0 transition-opacity hover:bg-[#f2f0eb] hover:text-danger group-hover:opacity-100"
            >
              <IconClose className="h-[11px] w-[11px]" />
            </button>
          </div>
        ))}
        <button
          onClick={() => onChange([...rows, emptyKV()])}
          className="flex h-[32px] w-full items-center px-3 text-[12px] text-muted transition-colors hover:text-ink"
        >
          + 添加
        </button>
      </div>
    </div>
  );
}

function NumField({
  label,
  value,
  onChange,
  min = 0,
  max = 100000,
  suffix,
}: {
  label: string;
  value: number;
  onChange: (v: number) => void;
  min?: number;
  max?: number;
  suffix?: string;
}) {
  return (
    <label className="flex items-center justify-between gap-3">
      <span className="whitespace-nowrap text-[11.5px] text-muted">{label}</span>
      <span className="flex items-center gap-1.5">
        <input
          type="number"
          value={value}
          min={min}
          max={max}
          onChange={(e) => {
            const n = Number(e.target.value);
            onChange(Number.isFinite(n) ? Math.min(max, Math.max(min, n)) : min);
          }}
          style={{ width: 86 }}
          className="h-[26px] rounded-[6px] border border-line bg-panel px-2 text-right font-mono text-[12px] text-ink outline-none focus:border-line-strong"
        />
        {suffix ? <span className="text-[11px] text-faint">{suffix}</span> : null}
      </span>
    </label>
  );
}

function TextField({
  label,
  value,
  onChange,
  placeholder,
  mono = true,
}: {
  label: string;
  value: string;
  onChange: (v: string) => void;
  placeholder?: string;
  mono?: boolean;
}) {
  return (
    <label className="flex items-center gap-3">
      <span className="w-[64px] shrink-0 text-[11.5px] text-muted">{label}</span>
      <input
        value={value}
        onChange={(e) => onChange(e.target.value)}
        spellCheck={false}
        autoCapitalize="off"
        autoCorrect="off"
        placeholder={placeholder}
        className={`h-[28px] min-w-0 flex-1 rounded-[6px] border border-line bg-panel px-2 text-[12px] text-ink outline-none placeholder:font-sans placeholder:text-faint focus:border-line-strong ${
          mono ? "font-mono" : ""
        }`}
      />
    </label>
  );
}

function ToolBtn({
  onClick,
  title,
  active,
  children,
}: {
  onClick?: () => void;
  title?: string;
  active?: boolean;
  children: React.ReactNode;
}) {
  return (
    <button
      onClick={onClick}
      title={title}
      className={`flex h-[30px] items-center gap-1.5 whitespace-nowrap rounded-[7px] border px-2.5 text-[11.5px] transition-colors ${
        active
          ? "border-accent/40 bg-accent-soft text-accent"
          : "border-line bg-panel text-muted hover:border-line-strong hover:text-ink"
      }`}
    >
      {children}
    </button>
  );
}

export default function HttpTool() {
  const [method, setMethod] = useState("GET");
  const [url, setUrl] = useState("");
  const [params, setParams] = useState<HttpKV[]>([emptyKV()]);
  const [headers, setHeaders] = useState<HttpKV[]>([emptyKV()]);
  const [bodyKind, setBodyKind] = useState<HttpBodyKind>("none");
  const [body, setBody] = useState("");
  const [bodyParams, setBodyParams] = useState<HttpKV[]>([emptyKV()]);
  const [contentType, setContentType] = useState("");
  const [authKind, setAuthKind] = useState<HttpAuthKind>("none");
  const [authToken, setAuthToken] = useState("");
  const [authUser, setAuthUser] = useState("");
  const [authPass, setAuthPass] = useState("");
  const [opts, setOpts] = useState<HttpOptions>(DEFAULTS);

  const [tab, setTab] = useState<Tab>("params");
  const [respTab, setRespTab] = useState<RespTab>("body");
  const [resp, setResp] = useState<HttpOutcome | null>(null);
  const [built, setBuilt] = useState<BuiltRequest | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [pretty, setPretty] = useState(true);
  const [curlStyle, setCurlStyle] = useState<CurlStyle>("bash");
  const [curlCopied, setCurlCopied] = useState(false);

  const [history, setHistory] = useState<HistoryItem[]>(loadHistory);
  const [historyOpen, setHistoryOpen] = useState(false);

  const spec = useMemo<HttpSpec>(
    () => ({
      method,
      url,
      params: params.filter((p) => p.name.trim() !== ""),
      headers: headers.filter((h) => h.name.trim() !== ""),
      bodyKind,
      body,
      bodyParams: bodyParams.filter((p) => p.name.trim() !== ""),
      contentType,
      authKind,
      authToken,
      authUser,
      authPass,
      options: opts,
    }),
    [method, url, params, headers, bodyKind, body, bodyParams, contentType, authKind, authToken, authUser, authPass, opts],
  );

  const specKey = JSON.stringify(spec);

  // 实时组装，供 cURL 预览与复制使用；输入不完整时静默失败，不弹红字
  useEffect(() => {
    let alive = true;
    const t = setTimeout(() => {
      api
        .httpBuild(spec)
        .then((b) => {
          if (alive) setBuilt(b);
        })
        .catch(() => {
          if (alive) setBuilt(null);
        });
    }, 300);
    return () => {
      alive = false;
      clearTimeout(t);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [specKey]);

  const curl = built ? toCurl(built, opts, curlStyle) : "";

  const remember = (s: HttpSpec) => {
    const item: HistoryItem = { at: Date.now(), method: s.method, url: s.url, spec: s };
    const next = [item, ...history.filter((h) => !(h.method === item.method && h.url === item.url))].slice(0, 20);
    setHistory(next);
    try {
      localStorage.setItem(HISTORY_KEY, JSON.stringify(next));
    } catch {
      /* 存不下就算了，历史只是顺手 */
    }
  };

  const send = async () => {
    if (busy) return;
    setBusy(true);
    setErr(null);
    try {
      const out = await api.httpSend(spec);
      setResp(out);
      setRespTab("body");
      remember(spec);
    } catch (e) {
      setErr(String(e));
      setResp(null);
    } finally {
      setBusy(false);
    }
  };

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === "Enter") {
        e.preventDefault();
        void send();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  const copyCurl = async () => {
    if (!curl) return;
    const ok = await copyText(curl);
    if (ok) {
      setCurlCopied(true);
      setTimeout(() => setCurlCopied(false), 1200);
    }
  };

  const applyHistory = (item: HistoryItem) => {
    const s = item.spec;
    setMethod(s.method);
    setUrl(s.url);
    setParams(s.params?.length ? s.params : [emptyKV()]);
    setHeaders(s.headers?.length ? s.headers : [emptyKV()]);
    setBodyKind(s.bodyKind ?? "none");
    setBody(s.body ?? "");
    setBodyParams(s.bodyParams?.length ? s.bodyParams : [emptyKV()]);
    setContentType(s.contentType ?? "");
    setAuthKind(s.authKind ?? "none");
    setAuthToken(s.authToken ?? "");
    setAuthUser(s.authUser ?? "");
    setAuthPass(s.authPass ?? "");
    setOpts({ ...DEFAULTS, ...(s.options ?? {}) });
    setHistoryOpen(false);
  };

  const extractParams = () => {
    const at = url.indexOf("?");
    if (at < 0) return;
    const frag = url.includes("#") ? url.slice(url.indexOf("#")) : "";
    const query = url.slice(at + 1).split("#")[0];
    const decode = (s: string) => {
      try {
        return decodeURIComponent(s.replace(/\+/g, " "));
      } catch {
        return s;
      }
    };
    const rows = query
      .split("&")
      .filter(Boolean)
      .map((pair) => {
        const eq = pair.indexOf("=");
        return eq < 0
          ? { name: decode(pair), value: "", enabled: true }
          : { name: decode(pair.slice(0, eq)), value: decode(pair.slice(eq + 1)), enabled: true };
      });
    setParams([...params.filter((p) => p.name.trim()), ...rows, emptyKV()]);
    setUrl(url.slice(0, at) + frag);
  };

  const shownBody = useMemo(() => {
    if (!resp) return "";
    if (resp.isBase64) return resp.body;
    if (!pretty || !looksJson(resp.contentType, resp.body)) return resp.body;
    try {
      return JSON.stringify(JSON.parse(resp.body), null, 2);
    } catch {
      return resp.body;
    }
  }, [resp, pretty]);

  const headerText = resp ? resp.headers.map((h) => `${h.name}: ${h.value}`).join("\n") : "";

  const statusTone =
    resp === null
      ? "text-muted"
      : resp.status >= 400
        ? "text-danger"
        : resp.status >= 300
          ? "text-warn"
          : "text-accent";

  const tabs: { id: Tab; label: string; count: number }[] = [
    { id: "params", label: "参数", count: params.filter((p) => p.name.trim()).length },
    { id: "headers", label: "请求头", count: headers.filter((h) => h.name.trim()).length },
    {
      id: "body",
      label: "请求体",
      count: bodyKind === "none" ? 0 : bodyKind === "raw" ? (body ? 1 : 0) : bodyParams.filter((p) => p.name.trim()).length,
    },
    { id: "auth", label: "认证", count: authKind === "none" ? 0 : 1 },
    { id: "options", label: "设置", count: 0 },
  ];

  const rightTabs: { id: RespTab; label: string }[] = [
    { id: "body", label: "响应体" },
    { id: "headers", label: "响应头" },
    { id: "curl", label: "cURL" },
  ];

  return (
    <div className="flex h-full flex-col">
      <div className="flex min-h-[52px] shrink-0 items-center gap-2 border-b border-line bg-page px-5 py-2">
        <div className="relative flex shrink-0 items-center">
          <select
            value={method}
            onChange={(e) => setMethod(e.target.value)}
            className="h-[32px] cursor-pointer appearance-none rounded-[7px] border border-line bg-panel pl-2.5 pr-6 font-mono text-[12.5px] font-medium text-ink outline-none hover:border-line-strong focus:border-line-strong"
          >
            {METHODS.map((m) => (
              <option key={m} value={m}>
                {m}
              </option>
            ))}
          </select>
          <IconChevron className="pointer-events-none absolute right-[6px] h-[10px] w-[10px] rotate-90 text-faint" />
        </div>

        <input
          value={url}
          onChange={(e) => setUrl(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") void send();
          }}
          spellCheck={false}
          autoCapitalize="off"
          autoCorrect="off"
          placeholder="https://api.example.com/users"
          className="h-[32px] min-w-0 flex-1 rounded-[7px] border border-line bg-panel px-3 font-mono text-[13px] text-ink outline-none placeholder:font-sans placeholder:text-faint focus:border-line-strong"
        />

        <div className="relative shrink-0">
          <ToolBtn onClick={() => setHistoryOpen((v) => !v)} active={historyOpen} title="最近请求">
            <IconHistory className="h-[13px] w-[13px]" />
            最近
          </ToolBtn>
          {historyOpen ? (
            <>
              <div className="fixed inset-0 z-20" onClick={() => setHistoryOpen(false)} />
              <div className="absolute right-0 top-[36px] z-30 w-[440px] overflow-hidden rounded-[9px] border border-line bg-panel shadow-[0_12px_32px_rgba(23,23,20,0.14)]">
                <div className="flex items-center justify-between border-b border-line px-3 py-1.5">
                  <span className="text-[11px] text-muted">最近请求</span>
                  <button
                    onClick={() => {
                      setHistory([]);
                      try {
                        localStorage.removeItem(HISTORY_KEY);
                      } catch {
                        /* ignore */
                      }
                      setHistoryOpen(false);
                    }}
                    className="rounded-[5px] px-1.5 py-0.5 text-[11.5px] text-muted transition-colors hover:bg-[#f2f0eb] hover:text-danger"
                  >
                    清空
                  </button>
                </div>
                <div className="max-h-[320px] overflow-auto">
                  {history.length === 0 ? (
                    <div className="px-3 py-5 text-center text-[12px] text-faint">空</div>
                  ) : (
                    history.map((h, i) => (
                      <button
                        key={`${h.at}-${i}`}
                        onClick={() => applyHistory(h)}
                        className="flex w-full items-center gap-2.5 border-b border-line px-3 py-[7px] text-left transition-colors last:border-b-0 hover:bg-[#faf9f7]"
                      >
                        <span className="w-[46px] shrink-0 font-mono text-[11px] text-accent">{h.method}</span>
                        <span className="min-w-0 flex-1 truncate font-mono text-[11.5px] text-ink">{h.url}</span>
                        <span className="shrink-0 text-[10.5px] text-faint">
                          {new Date(h.at).toLocaleTimeString("zh-CN", { hour12: false })}
                        </span>
                      </button>
                    ))
                  )}
                </div>
              </div>
            </>
          ) : null}
        </div>

        <ToolBtn onClick={() => void copyCurl()} title={curl || "URL 还不完整"} active={curlCopied}>
          <IconTerminal className="h-[13px] w-[13px]" />
          {curlCopied ? "已复制" : "复制 cURL"}
        </ToolBtn>

        <PrimaryBtn onClick={() => void send()} disabled={busy}>
          <span className="flex items-center gap-1.5">
            <IconSend className="h-[12px] w-[12px]" />
            {busy ? "发送中…" : "发送"}
          </span>
        </PrimaryBtn>
      </div>

      <div className="flex min-h-0 flex-1 gap-px bg-line">
        <Pane
          grow={1.15}
          label={
            <div className="flex items-center gap-0.5">
              {tabs.map((t) => (
                <button
                  key={t.id}
                  onClick={() => setTab(t.id)}
                  className={`rounded-[5px] px-2 py-[3px] text-[11.5px] transition-colors ${
                    tab === t.id ? "bg-[#f1efea] text-ink" : "text-muted hover:text-ink"
                  }`}
                >
                  {t.label}
                  {t.count > 0 ? <span className="ml-1 text-[10px] text-faint">{t.count}</span> : null}
                </button>
              ))}
            </div>
          }
          actions={
            tab === "params" && url.includes("?") ? (
              <button
                onClick={extractParams}
                className="rounded-[5px] px-1.5 py-1 text-[11px] text-muted transition-colors hover:bg-[#f2f0eb] hover:text-ink"
              >
                从 URL 提取
              </button>
            ) : null
          }
        >
          {tab === "params" ? (
            <KVEditor rows={params} onChange={setParams} keyPlaceholder="参数名" valuePlaceholder="参数值" />
          ) : null}

          {tab === "headers" ? (
            <KVEditor rows={headers} onChange={setHeaders} keyPlaceholder="Header" valuePlaceholder="值" />
          ) : null}

          {tab === "body" ? (
            <div className="flex h-full flex-col">
              <div className="flex h-[38px] shrink-0 items-center gap-2 border-b border-line px-3">
                <Segmented
                  value={bodyKind}
                  onChange={setBodyKind}
                  options={[
                    { value: "none", label: "无" },
                    { value: "raw", label: "Raw" },
                    { value: "form", label: "Form" },
                    { value: "urlencoded", label: "URL Encoded" },
                  ]}
                />
                {bodyKind === "raw" ? (
                  <>
                    <input
                      value={contentType}
                      onChange={(e) => setContentType(e.target.value)}
                      spellCheck={false}
                      placeholder="Content-Type（留空自动）"
                      list="ikt-ct-presets"
                      className="ml-1 h-[26px] min-w-0 flex-1 rounded-[6px] border border-line bg-panel px-2 font-mono text-[11.5px] text-ink outline-none placeholder:font-sans placeholder:text-faint focus:border-line-strong"
                    />
                    <datalist id="ikt-ct-presets">
                      {CONTENT_TYPES.map((ct) => (
                        <option key={ct} value={ct} />
                      ))}
                    </datalist>
                  </>
                ) : null}
              </div>
              <div className="min-h-0 flex-1">
                {bodyKind === "none" ? (
                  <div className="flex h-full items-center justify-center text-[12px] text-faint">无请求体</div>
                ) : bodyKind === "raw" ? (
                  <CodeEditor
                    value={body}
                    onChange={setBody}
                    language={contentType.includes("json") || looksJson(contentType, body) ? "json" : "text"}
                    placeholder="请求体"
                  />
                ) : (
                  <KVEditor
                    rows={bodyParams}
                    onChange={setBodyParams}
                    keyPlaceholder="字段名"
                    valuePlaceholder="字段值"
                  />
                )}
              </div>
            </div>
          ) : null}

          {tab === "auth" ? (
            <div className="h-full overflow-auto px-4 py-3">
              <Segmented
                value={authKind}
                onChange={setAuthKind}
                options={[
                  { value: "none", label: "无" },
                  { value: "bearer", label: "Bearer" },
                  { value: "basic", label: "Basic" },
                ]}
              />
              <div className="mt-4 flex max-w-[520px] flex-col gap-2.5">
                {authKind === "bearer" ? (
                  <TextField label="Token" value={authToken} onChange={setAuthToken} placeholder="eyJhbGciOi…" />
                ) : null}
                {authKind === "basic" ? (
                  <>
                    <TextField label="用户名" value={authUser} onChange={setAuthUser} placeholder="user" />
                    <TextField label="密码" value={authPass} onChange={setAuthPass} placeholder="password" />
                  </>
                ) : null}
                {authKind === "none" ? <span className="text-[12px] text-faint">不使用</span> : null}
              </div>
            </div>
          ) : null}

          {tab === "options" ? (
            <div className="h-full overflow-auto px-4 py-3">
              <div className="grid max-w-[560px] grid-cols-2 gap-x-8 gap-y-2.5">
                <NumField
                  label="超时"
                  value={opts.timeoutMs}
                  min={1}
                  max={300_000}
                  suffix="ms"
                  onChange={(v) => setOpts({ ...opts, timeoutMs: v })}
                />
                <NumField
                  label="最大重定向"
                  value={opts.maxRedirects}
                  min={0}
                  max={20}
                  onChange={(v) => setOpts({ ...opts, maxRedirects: v })}
                />
                <NumField
                  label="响应上限"
                  value={Math.round(opts.maxBody / 1024 / 1024)}
                  min={1}
                  max={64}
                  suffix="MB"
                  onChange={(v) => setOpts({ ...opts, maxBody: v * 1024 * 1024 })}
                />
              </div>
              <div className="mt-4 flex items-center gap-5">
                <Toggle checked={opts.followRedirects} onChange={(v) => setOpts({ ...opts, followRedirects: v })}>
                  跟随重定向
                </Toggle>
                <Toggle checked={opts.verifyTls} onChange={(v) => setOpts({ ...opts, verifyTls: v })}>
                  校验 TLS 证书
                </Toggle>
              </div>

              <div className="mt-5 max-w-[560px] border-t border-line pt-4">
                <div className="flex items-center gap-2.5">
                  <Toggle
                    checked={opts.proxyEnabled}
                    onChange={(v) => setOpts({ ...opts, proxyEnabled: v })}
                  >
                    代理
                  </Toggle>
                  <input
                    value={opts.proxyUrl}
                    onChange={(e) => setOpts({ ...opts, proxyUrl: e.target.value })}
                    disabled={!opts.proxyEnabled}
                    spellCheck={false}
                    placeholder="http://127.0.0.1:8080"
                    className="h-[28px] min-w-0 flex-1 rounded-[6px] border border-line bg-panel px-2 font-mono text-[12px] text-ink outline-none placeholder:font-sans placeholder:text-faint focus:border-line-strong disabled:opacity-45"
                  />
                </div>
                <div className="mt-2 flex items-center gap-2.5 pl-[52px]">
                  <input
                    value={opts.proxyUser}
                    onChange={(e) => setOpts({ ...opts, proxyUser: e.target.value })}
                    disabled={!opts.proxyEnabled}
                    spellCheck={false}
                    autoComplete="off"
                    placeholder="用户名"
                    className="h-[28px] w-[150px] rounded-[6px] border border-line bg-panel px-2 font-mono text-[12px] text-ink outline-none placeholder:font-sans placeholder:text-faint focus:border-line-strong disabled:opacity-45"
                  />
                  <input
                    value={opts.proxyPass}
                    onChange={(e) => setOpts({ ...opts, proxyPass: e.target.value })}
                    disabled={!opts.proxyEnabled}
                    spellCheck={false}
                    autoComplete="new-password"
                    type="password"
                    placeholder="密码"
                    className="h-[28px] w-[150px] rounded-[6px] border border-line bg-panel px-2 font-mono text-[12px] text-ink outline-none placeholder:font-sans placeholder:text-faint focus:border-line-strong disabled:opacity-45"
                  />
                </div>
              </div>
            </div>
          ) : null}
        </Pane>

        <Pane
          grow={1}
          label={
            resp ? (
              <>
                <span className={`font-mono text-[12px] ${statusTone}`}>
                  {resp.status} {resp.statusText}
                </span>
                <span className="font-normal text-faint">
                  {resp.elapsedMs.toFixed(0)} ms · {bytes(resp.bodyBytes)}
                  {resp.truncated ? " · 已截断" : ""}
                </span>
              </>
            ) : (
              <span className="font-normal text-faint">{busy ? "请求中…" : "未发送"}</span>
            )
          }
          actions={
            <div className="flex items-center gap-0.5">
              {rightTabs.map((t) => (
                <button
                  key={t.id}
                  onClick={() => setRespTab(t.id)}
                  className={`rounded-[5px] px-2 py-[3px] text-[11.5px] transition-colors ${
                    respTab === t.id ? "bg-[#f1efea] text-ink" : "text-muted hover:text-ink"
                  }`}
                >
                  {t.label}
                </button>
              ))}
              <button
                onClick={() => void copyText(respTab === "curl" ? curl : respTab === "headers" ? headerText : shownBody)}
                className="ml-1 rounded-[5px] px-1.5 py-1 text-[11px] text-muted transition-colors hover:bg-[#f2f0eb] hover:text-ink"
              >
                复制
              </button>
            </div>
          }
        >
          <div className="h-full overflow-hidden">
            {respTab === "curl" ? (
              <div className="flex h-full flex-col">
                <div className="flex h-[38px] shrink-0 items-center gap-2 border-b border-line px-3">
                  <Segmented
                    value={curlStyle}
                    onChange={setCurlStyle}
                    options={[
                      { value: "bash", label: "Bash" },
                      { value: "bashInline", label: "单行" },
                      { value: "powershell", label: "PowerShell" },
                    ]}
                  />
                </div>
                <div className="min-h-0 flex-1">
                  <CodeEditor value={curl} language="text" readOnly placeholder="URL 还不完整" />
                </div>
              </div>
            ) : err ? (
              <div className="p-4">
                <div className="rounded-[8px] border border-[#e6c3bb] bg-danger-soft p-3 text-[12px] leading-relaxed text-[#7d2e24]">
                  {err}
                </div>
              </div>
            ) : !resp ? (
              <div className="flex h-[180px] items-center justify-center text-[12px] text-faint">
                {busy ? "请求中…" : "未发送"}
              </div>
            ) : respTab === "headers" ? (
              <div className="h-full overflow-auto">
                {resp.headers.map((h, i) => (
                  <div
                    key={`${h.name}-${i}`}
                    className="grid grid-cols-[188px_1fr] items-start gap-3 border-b border-line px-4 py-[7px]"
                  >
                    <span className="break-all font-mono text-[11.5px] text-muted">{h.name}</span>
                    <span className="select-all break-all font-mono text-[12px] text-ink">{h.value}</span>
                  </div>
                ))}
              </div>
            ) : resp.isBase64 && resp.contentType.startsWith("image/") ? (
              <div className="flex h-full items-center justify-center overflow-auto bg-[#faf9f7] p-4">
                <img
                  alt=""
                  src={`data:${resp.contentType};base64,${resp.body}`}
                  className="max-h-full max-w-full object-contain"
                />
              </div>
            ) : resp.isBase64 ? (
              <div className="flex h-full flex-col">
                <div className="flex h-[32px] shrink-0 items-center gap-2 border-b border-line px-3 text-[11px] text-muted">
                  二进制 · Base64
                  <span className="text-faint">{resp.contentType}</span>
                </div>
                <div className="min-h-0 flex-1">
                  <CodeEditor value={resp.body} language="text" readOnly />
                </div>
              </div>
            ) : (
              <div className="flex h-full flex-col">
                {looksJson(resp.contentType, resp.body) ? (
                  <div className="flex h-[32px] shrink-0 items-center gap-2 border-b border-line px-3">
                    <button
                      onClick={() => setPretty((v) => !v)}
                      className={`rounded-[5px] px-1.5 py-[2px] text-[11px] transition-colors ${
                        pretty ? "bg-accent-soft text-accent" : "text-muted hover:text-ink"
                      }`}
                    >
                      美化
                    </button>
                    <span className="font-mono text-[11px] text-faint">{resp.contentType}</span>
                  </div>
                ) : null}
                <div className="min-h-0 flex-1">
                  <CodeEditor
                    value={shownBody}
                    language={looksJson(resp.contentType, resp.body) && pretty ? "json" : "text"}
                    readOnly
                  />
                </div>
              </div>
            )}
          </div>
        </Pane>
      </div>
    </div>
  );
}
