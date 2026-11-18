import { useEffect, useState } from "react";
import CodeEditor from "../components/CodeEditor";
import { Pane, PrimaryBtn, Segmented, Toggle } from "../components/ui";
import { api, type DnsOutcome, type GeoOutcome, type PingOutcome, type ProbeOutcome } from "../lib/api";

type Mode = "ping" | "telnet" | "dns" | "geo";

function Num({
  label,
  value,
  onChange,
  min = 1,
  max = 100000,
  width = 62,
}: {
  label: string;
  value: number;
  onChange: (v: number) => void;
  min?: number;
  max?: number;
  width?: number;
}) {
  return (
    <label className="flex items-center gap-1.5">
      <span className="whitespace-nowrap text-[11.5px] text-muted">{label}</span>
      <input
        type="number"
        value={value}
        min={min}
        max={max}
        onChange={(e) => {
          const n = Number(e.target.value);
          onChange(Number.isFinite(n) ? n : min);
        }}
        style={{ width }}
        className="h-[26px] rounded-[6px] border border-line bg-panel px-2 font-mono text-[12px] text-ink outline-none focus:border-line-strong"
      />
    </label>
  );
}

function HostInput({
  value,
  onChange,
  placeholder,
}: {
  value: string;
  onChange: (v: string) => void;
  placeholder: string;
}) {
  return (
    <input
      value={value}
      onChange={(e) => onChange(e.target.value)}
      onKeyDown={(e) => {
        if (e.key === "Enter") (e.target as HTMLInputElement).blur();
      }}
      spellCheck={false}
      autoCapitalize="off"
      autoCorrect="off"
      placeholder={placeholder}
      className="h-[26px] min-w-[150px] flex-1 rounded-[6px] border border-line bg-panel px-2 font-mono text-[12.5px] text-ink outline-none placeholder:font-sans placeholder:text-faint focus:border-line-strong"
    />
  );
}

function Row({ label, value, mono = true }: { label: string; value: string; mono?: boolean }) {
  return (
    <div className="grid w-full grid-cols-[132px_1fr] items-start gap-3 border-b border-line px-5 py-[9px]">
      <span className="text-[11.5px] text-muted">{label}</span>
      <span
        className={`select-all whitespace-pre-wrap break-all text-[12.5px] text-ink ${mono ? "font-mono" : ""}`}
      >
        {value}
      </span>
    </div>
  );
}

export default function NetTool() {
  const [mode, setMode] = useState<Mode>("ping");
  const [host, setHost] = useState("");
  const [port, setPort] = useState(443);
  const [count, setCount] = useState(4);
  const [timeoutMs, setTimeoutMs] = useState(2000);
  const [readMs, setReadMs] = useState(1200);
  const [payload, setPayload] = useState("");
  const [newline, setNewline] = useState(true);

  const [ping, setPing] = useState<PingOutcome | null>(null);
  const [probe, setProbe] = useState<ProbeOutcome | null>(null);
  const [dns, setDns] = useState<DnsOutcome | null>(null);
  const [geo, setGeo] = useState<GeoOutcome | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    setPing(null);
    setProbe(null);
    setDns(null);
    setGeo(null);
    setErr(null);
    setBusy(false);
  }, [mode, host, port, count, timeoutMs, readMs, payload, newline]);

  const run = async () => {
    if (busy) return;
    setBusy(true);
    try {
      if (mode === "ping") {
        setPing(await api.netPing({ host, port, count, timeoutMs }));
      } else if (mode === "telnet") {
        setProbe(
          await api.netProbe({
            host,
            port,
            payload,
            appendNewline: newline,
            timeoutMs,
            readMs,
          }),
        );
      } else if (mode === "dns") {
        setDns(await api.netDns(host));
      } else {
        setGeo(await api.netGeo(host));
      }
      setErr(null);
    } catch (e) {
      setErr(String(e));
    } finally {
      setBusy(false);
    }
  };

  const errBox = (
    <div className="p-4">
      <div className="rounded-[8px] border border-[#e6c3bb] bg-danger-soft p-3 text-[12px] leading-relaxed text-[#7d2e24]">
        {err}
      </div>
    </div>
  );

  const empty = (text: string) => (
    <div className="flex h-[180px] items-center justify-center text-[12px] text-faint">{text}</div>
  );

  const status =
    mode === "ping" && ping
      ? `${ping.received}/${ping.sent} 通 · 丢包 ${ping.lossPct}% · ${ping.avgMs} ms 平均`
      : mode === "telnet" && probe
        ? `${probe.addr} · 连接 ${probe.connectMs} ms · 收 ${probe.recvBytes} B${probe.closed ? " · 已关闭" : ""}`
        : mode === "dns" && dns
          ? `${dns.addrs.length} 条记录 · ${(dns.elapsedUs / 1000).toFixed(0)} ms`
          : null;

  return (
    <div className="flex h-full flex-col">
      <div className="flex min-h-[44px] shrink-0 flex-wrap items-center gap-2.5 border-b border-line bg-page px-5 py-1.5">
        <Segmented
          value={mode}
          onChange={setMode}
          options={[
            { value: "ping", label: "TCP Ping" },
            { value: "telnet", label: "Telnet" },
            { value: "dns", label: "DNS" },
            { value: "geo", label: "IP 归属地" },
          ]}
        />
        <span className="h-[15px] w-px bg-line" />
        <HostInput
          value={host}
          onChange={setHost}
          placeholder={mode === "geo" ? "8.8.8.8 或域名" : "域名或 IP"}
        />
        {mode === "ping" || mode === "telnet" ? (
          <Num label="端口" value={port} onChange={setPort} max={65535} />
        ) : null}
        {mode === "ping" ? (
          <>
            <Num label="次数" value={count} onChange={setCount} max={20} />
            <Num label="超时" value={timeoutMs} onChange={setTimeoutMs} max={30000} width={68} />
          </>
        ) : null}
        {mode === "telnet" ? (
          <>
            <Num label="超时" value={timeoutMs} onChange={setTimeoutMs} max={30000} width={68} />
            <Num label="读" value={readMs} onChange={setReadMs} max={30000} width={68} />
          </>
        ) : null}
        <div className="ml-auto flex items-center gap-2.5">
          <PrimaryBtn onClick={() => void run()} disabled={busy}>
            {busy ? "探测中…" : mode === "dns" ? "解析" : mode === "geo" ? "查询" : "开始"}
          </PrimaryBtn>
        </div>
      </div>

      {mode === "telnet" ? (
        <div className="flex h-[38px] shrink-0 items-center gap-2 border-b border-line bg-page px-5">
          <span className="whitespace-nowrap text-[11.5px] text-muted">发送</span>
          <input
            value={payload}
            onChange={(e) => setPayload(e.target.value)}
            spellCheck={false}
            placeholder="留空则只收不发"
            className="min-w-0 flex-1 bg-transparent font-mono text-[12px] text-ink outline-none placeholder:font-sans placeholder:text-faint"
          />
          <Toggle checked={newline} onChange={setNewline}>
            追加 CRLF
          </Toggle>
        </div>
      ) : null}

      <div className="flex min-h-0 flex-1 gap-px bg-line">
        {mode === "telnet" ? (
          <>
            <Pane
              label={
                <>
                  文本
                  {status ? <span className="font-normal text-faint">{status}</span> : null}
                </>
              }
            >
              {err ? (
                errBox
              ) : (
                <CodeEditor value={probe?.text ?? ""} language="text" readOnly placeholder="收到的内容" />
              )}
            </Pane>
            <Pane label={<>HEX{probe ? <span className="font-normal text-faint">{probe.recvBytes} B</span> : null}</>}>
              <CodeEditor value={probe?.hex ?? ""} language="text" readOnly placeholder="十六进制视图" />
            </Pane>
          </>
        ) : (
          <Pane
            label={
              <>
                {mode === "ping" ? "探测" : mode === "dns" ? "解析结果" : "归属地"}
                {status ? <span className="font-normal text-faint">{status}</span> : null}
              </>
            }
          >
            <div className="h-full overflow-auto">
              {err
                ? errBox
                : mode === "ping" && ping
                  ? ping.samples.map((s) => (
                      <div
                        key={s.seq}
                        className="flex items-center gap-3 border-b border-line px-5 py-[7px] font-mono text-[12px]"
                      >
                        <span className="w-[28px] text-faint">#{s.seq}</span>
                        <span className={s.ms === null ? "text-danger" : "text-ink"}>
                          {s.ms === null ? (s.error ?? "失败") : `${s.ms} ms`}
                        </span>
                      </div>
                    ))
                  : mode === "dns" && dns
                    ? dns.addrs.map((a) => (
                        <Row key={a.ip} label={a.family} value={a.ip} />
                      ))
                    : mode === "geo" && geo
                      ? [
                          { label: "IP", value: geo.ip },
                          { label: "协议", value: geo.version },
                          { label: "国家", value: geo.country },
                          { label: "省份", value: geo.province },
                          { label: "城市", value: geo.city },
                          { label: "运营商", value: geo.isp },
                          { label: "原始", value: geo.raw },
                        ]
                          .filter((r) => r.value)
                          .map((r) => <Row key={r.label} label={r.label} value={r.value} />)
                      : empty(mode === "ping" ? "还没探测" : mode === "dns" ? "还没解析" : "还没查询")}
            </div>
          </Pane>
        )}
      </div>
    </div>
  );
}
