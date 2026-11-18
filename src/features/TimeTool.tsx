import { useEffect, useState } from "react";
import { IconCheck, IconCopy } from "../components/icons";
import { Select, TextBtn, copyText } from "../components/ui";
import { api, type TimeOutcome } from "../lib/api";

const UNITS: { value: string; label: string }[] = [
  { value: "auto", label: "自动识别" },
  { value: "s", label: "秒" },
  { value: "ms", label: "毫秒" },
  { value: "us", label: "微秒" },
  { value: "ns", label: "纳秒" },
];

const ZONES: { value: string; label: string }[] = [
  { value: "+08:00", label: "+08:00 北京" },
  { value: "Z", label: "+00:00 UTC" },
  { value: "+09:00", label: "+09:00 东京" },
  { value: "+05:30", label: "+05:30 印度" },
  { value: "+01:00", label: "+01:00 柏林" },
  { value: "-05:00", label: "-05:00 纽约" },
  { value: "-08:00", label: "-08:00 洛杉矶" },
];

function Row({ label, value }: { label: string; value: string }) {
  const [done, setDone] = useState(false);
  return (
    <button
      onClick={async () => {
        if (!value) return;
        if (await copyText(value)) {
          setDone(true);
          setTimeout(() => setDone(false), 1000);
        }
      }}
      className="group grid w-full grid-cols-[132px_1fr_20px] items-center gap-3 border-b border-line px-5 py-[9px] text-left hover:bg-[#f7f5f1]"
    >
      <span className="text-[11.5px] text-muted">{label}</span>
      <span className="select-all whitespace-pre-wrap break-all font-mono text-[12.5px] text-ink">
        {value}
      </span>
      <span className="text-faint opacity-0 transition-opacity group-hover:opacity-100">
        {done ? <IconCheck className="h-[13px] w-[13px]" /> : <IconCopy className="h-[13px] w-[13px]" />}
      </span>
    </button>
  );
}

export default function TimeTool() {
  const [input, setInput] = useState("");
  const [unit, setUnit] = useState("auto");
  const [tz, setTz] = useState("+08:00");
  const [out, setOut] = useState<TimeOutcome | null>(null);
  const [err, setErr] = useState<string | null>(null);

  useEffect(() => {
    let alive = true;
    if (!input.trim()) {
      setOut(null);
      setErr(null);
      return;
    }
    const timer = setTimeout(() => {
      api
        .timeConvert({ tz, unit }, input)
        .then((r) => {
          if (!alive) return;
          setOut(r);
          setErr(null);
        })
        .catch((e) => {
          if (!alive) return;
          setOut(null);
          setErr(String(e));
        });
    }, 120);
    return () => {
      alive = false;
      clearTimeout(timer);
    };
  }, [input, tz, unit]);

  const rows = out
    ? [
        { label: "秒", value: String(out.seconds) },
        { label: "毫秒", value: String(out.millis) },
        { label: "微秒", value: String(out.micros) },
        { label: "纳秒", value: String(out.nanos) },
        { label: "UTC", value: out.utcIso },
        { label: `本地 ${out.tzLabel}`, value: out.localIso },
        { label: "本地写法", value: out.localText },
        { label: "周", value: `${out.weekday} · 第 ${out.dayOfYear} 天 · 第 ${out.week} 周` },
        { label: "相对现在", value: out.relative },
      ]
    : [];

  const status = err ? (
    <span className="text-faint">解析失败</span>
  ) : out ? (
    <span className="font-normal text-faint">
      {out.direction === "toDate" ? `按${out.unit === "s" ? "秒" : out.unit === "ms" ? "毫秒" : out.unit === "us" ? "微秒" : "纳秒"}解读` : "按日期解读"}{" "}
      · {out.tzLabel}
    </span>
  ) : null;

  return (
    <div className="flex h-full flex-col">
      <div className="flex h-[44px] shrink-0 items-center gap-2 border-b border-line bg-page px-5">
        <input
          value={input}
          onChange={(e) => setInput(e.target.value)}
          spellCheck={false}
          autoCapitalize="off"
          autoCorrect="off"
          placeholder="1700000000 或 2026-11-17 14:24:00"
          className="h-[30px] min-w-0 flex-1 rounded-[7px] border border-line bg-panel px-2.5 font-mono text-[13px] text-ink outline-none placeholder:font-sans placeholder:text-faint focus:border-line-strong"
        />
        <TextBtn onClick={() => setInput(String(Date.now()))}>现在</TextBtn>
        <button
          onClick={() => setInput("")}
          className="rounded-[6px] px-2 py-1 text-[12px] text-muted transition-colors hover:bg-[#f2f0eb] hover:text-ink"
        >
          清空
        </button>
        <span className="h-[15px] w-px bg-line" />
        <span className="text-[11.5px] text-muted">单位</span>
        <Select value={unit} onChange={setUnit} options={UNITS} title="仅在输入是纯数字时生效" />
        <span className="text-[11.5px] text-muted">时区</span>
        <Select value={tz} onChange={setTz} options={ZONES} title="输入不含偏移时按这个时区解释" />
      </div>

      <div className="min-h-0 flex-1 overflow-auto bg-panel">
        <div className="flex h-[34px] items-center gap-2 border-b border-line pl-5 pr-4 text-[11px] font-medium tracking-wide text-muted">
          时间
          {status}
        </div>
        {err ? (
          <div className="p-4">
            <div className="rounded-[8px] border border-[#e6c3bb] bg-danger-soft p-3 text-[12px] leading-relaxed text-[#7d2e24]">
              {err}
            </div>
          </div>
        ) : rows.length ? (
          rows.map((r) => <Row key={r.label} label={r.label} value={r.value} />)
        ) : (
          <div className="flex h-[180px] items-center justify-center text-[12px] text-faint">
            输入时间戳或日期
          </div>
        )}
      </div>
    </div>
  );
}
