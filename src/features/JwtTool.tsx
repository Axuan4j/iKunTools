import { useEffect, useState, type ReactNode } from "react";
import CodeEditor from "../components/CodeEditor";
import { CopyBtn, Pane } from "../components/ui";
import { api, type JwtParts } from "../lib/api";

function humanLeft(sec: number): string {
  const s = Math.abs(sec);
  const d = Math.floor(s / 86400);
  const h = Math.floor((s % 86400) / 3600);
  const m = Math.floor((s % 3600) / 60);
  if (d > 0) return `${d} 天 ${h} 小时`;
  if (h > 0) return `${h} 小时 ${m} 分`;
  if (m > 0) return `${m} 分`;
  return `${s} 秒`;
}

function Tag({ tone, children }: { tone: "neutral" | "ok" | "bad"; children: ReactNode }) {
  const cls =
    tone === "ok"
      ? "border-[#bcdcd0] bg-accent-soft text-accent"
      : tone === "bad"
        ? "border-[#e6c3bb] bg-danger-soft text-danger"
        : "border-line bg-[#f6f4f0] text-muted";
  return (
    <span className={`rounded-[5px] border px-1.5 py-[1px] font-mono text-[10.5px] ${cls}`}>{children}</span>
  );
}

export default function JwtTool() {
  const [token, setToken] = useState("");
  const [parts, setParts] = useState<JwtParts | null>(null);
  const [err, setErr] = useState<string | null>(null);

  useEffect(() => {
    let alive = true;
    if (!token.trim()) {
      setParts(null);
      setErr(null);
      return;
    }
    const timer = setTimeout(() => {
      api
        .jwtDecode(token)
        .then((r) => {
          if (!alive) return;
          setParts(r);
          setErr(null);
        })
        .catch((e) => {
          if (!alive) return;
          setParts(null);
          setErr(String(e));
        });
    }, 140);
    return () => {
      alive = false;
      clearTimeout(timer);
    };
  }, [token]);

  return (
    <div className="flex h-full flex-col">
      <div className="flex min-h-0 flex-1 gap-px bg-line">
        <Pane
          label={<>令牌</>}
        >
          <CodeEditor
            value={token}
            onChange={setToken}
            language="text"
            placeholder="粘贴 JWT…"
          />
        </Pane>

        <Pane label={<>解析结果</>} actions={<CopyBtn text={parts?.payload ?? ""} label="复制 payload" />}>
          {err ? (
            <div className="p-4">
              <div className="rounded-[8px] border border-[#e6c3bb] bg-danger-soft p-3 text-[12px] leading-relaxed text-[#7d2e24]">
                {err}
              </div>
            </div>
          ) : !parts ? null : (
            <div className="flex h-full flex-col">
              <div className="flex flex-wrap items-center gap-1.5 border-b border-line px-3.5 py-2">
                <Tag tone="neutral">{parts.algorithm}</Tag>
                <Tag tone="neutral">{parts.typ}</Tag>
                {parts.exp === null ? (
                  <Tag tone="neutral">无 exp</Tag>
                ) : parts.expired ? (
                  <Tag tone="bad">已过期 {humanLeft(parts.secondsLeft ?? 0)}</Tag>
                ) : (
                  <Tag tone="ok">还有 {humanLeft(parts.secondsLeft ?? 0)}</Tag>
                )}
                {parts.missingSignature ? <Tag tone="bad">缺少签名段</Tag> : <Tag tone="neutral">签名已解析</Tag>}
                <Tag tone="neutral">未验签</Tag>
              </div>

              <div className="border-b border-line px-3.5 py-2">
                <div className="mb-1 flex items-center justify-between">
                  <span className="text-[10.5px] font-medium tracking-[0.04em] text-faint">HEADER</span>
                  <CopyBtn text={parts.header} label="复制" />
                </div>
                <pre className="max-h-[120px] overflow-auto rounded-[6px] bg-[#faf9f6] px-2.5 py-2 font-mono text-[11.5px] leading-[1.7] text-ink/85">
                  {parts.header}
                </pre>
              </div>

              <div className="flex min-h-0 flex-1 flex-col">
                <div className="flex items-center justify-between px-3.5 pb-1 pt-2">
                  <span className="text-[10.5px] font-medium tracking-[0.04em] text-faint">PAYLOAD</span>
                  {parts.iatText ? (
                    <span className="font-mono text-[10.5px] text-faint">iat {parts.iatText}</span>
                  ) : null}
                </div>
                <div className="min-h-0 flex-1">
                  <CodeEditor value={parts.payload} language="json" readOnly />
                </div>
              </div>

              <div className="shrink-0 border-t border-line px-3.5 py-1.5">
                <span className="font-mono text-[10.5px] break-all text-faint">
                  sig {parts.signature ? `${parts.signature.slice(0, 32)}…` : "（空）"}
                </span>
              </div>
            </div>
          )}
        </Pane>
      </div>
    </div>
  );
}
