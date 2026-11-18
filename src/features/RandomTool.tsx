import { useEffect, useState } from "react";
import CodeEditor from "../components/CodeEditor";
import { CopyBtn, Pane, PrimaryBtn, Select, Toggle } from "../components/ui";
import { api, type RandomKind, type RandomOutcome, type RandomSpec } from "../lib/api";

const KINDS: { value: RandomKind; label: string }[] = [
  { value: "uuid4", label: "UUID v4" },
  { value: "uuid7", label: "UUID v7" },
  { value: "ulid", label: "ULID" },
  { value: "nanoid", label: "NanoID" },
  { value: "password", label: "随机密码" },
  { value: "text", label: "随机文本" },
  { value: "bytes", label: "随机字节" },
  { value: "int", label: "随机整数" },
  { value: "pick", label: "清单抽取" },
];

const BYTE_ENCODINGS: { value: RandomSpec["bytesEncoding"]; label: string }[] = [
  { value: "hex", label: "HEX" },
  { value: "base64", label: "Base64" },
];

function Num({
  label,
  value,
  onChange,
  min = 1,
  max = 4096,
  width = 58,
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

export default function RandomTool() {
  const [kind, setKind] = useState<RandomKind>("uuid4");
  const [count, setCount] = useState(1);
  const [length, setLength] = useState(16);
  const [bytesLen, setBytesLen] = useState(16);
  const [bytesEncoding, setBytesEncoding] = useState<RandomSpec["bytesEncoding"]>("hex");
  const [alphabet, setAlphabet] = useState("");
  const [lower, setLower] = useState(true);
  const [upper, setUpper] = useState(true);
  const [digits, setDigits] = useState(true);
  const [symbols, setSymbols] = useState(false);
  const [noAmbiguous, setNoAmbiguous] = useState(true);
  const [min, setMin] = useState(1);
  const [max, setMax] = useState(100);
  const [pool, setPool] = useState("");

  const [out, setOut] = useState<RandomOutcome | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  // 参数一动就作废上一轮结果，避免把旧列表当成新参数生成的
  useEffect(() => {
    setOut(null);
    setErr(null);
  }, [kind, count, length, bytesLen, bytesEncoding, alphabet, lower, upper, digits, symbols, noAmbiguous, min, max, pool]);

  const run = async () => {
    if (busy) return;
    setBusy(true);
    try {
      const spec: RandomSpec = {
        kind,
        count,
        length,
        alphabet,
        digits,
        lower,
        upper,
        symbols,
        excludeAmbiguous: noAmbiguous,
        bytesLen,
        bytesEncoding,
        min,
        max,
        pool,
      };
      const r = await api.randomGenerate(spec);
      setOut(r);
      setErr(null);
    } catch (e) {
      setOut(null);
      setErr(String(e));
    } finally {
      setBusy(false);
    }
  };

  const params = (
    <>
      <Select value={kind} onChange={setKind} options={KINDS} />
      <span className="h-[15px] w-px bg-line" />
      <Num label="条数" value={count} onChange={setCount} max={1000} />

      {kind === "nanoid" || kind === "password" || kind === "text" ? (
        <Num label="长度" value={length} onChange={setLength} />
      ) : null}

      {kind === "bytes" ? (
        <>
          <Num label="字节" value={bytesLen} onChange={setBytesLen} />
          <Select value={bytesEncoding} onChange={setBytesEncoding} options={BYTE_ENCODINGS} />
        </>
      ) : null}

      {kind === "int" ? (
        <>
          <Num label="最小" value={min} onChange={setMin} min={-9007199254740991} width={86} />
          <Num label="最大" value={max} onChange={setMax} max={9007199254740991} width={86} />
        </>
      ) : null}

      {kind === "password" ? (
        <>
          <Toggle checked={lower} onChange={setLower}>
            小写
          </Toggle>
          <Toggle checked={upper} onChange={setUpper}>
            大写
          </Toggle>
          <Toggle checked={digits} onChange={setDigits}>
            数字
          </Toggle>
          <Toggle checked={symbols} onChange={setSymbols}>
            符号
          </Toggle>
          <Toggle checked={noAmbiguous} onChange={setNoAmbiguous}>
            排除易混
          </Toggle>
        </>
      ) : null}
    </>
  );

  const stats = out
    ? `${out.count} 条${out.bits > 0 ? ` · 熵 ${out.bits.toFixed(1)} bit` : ""} · ${(out.elapsedUs / 1000).toFixed(2)} ms`
    : busy
      ? "生成中…"
      : null;

  return (
    <div className="flex h-full flex-col">
      <div className="flex min-h-[44px] shrink-0 flex-wrap items-center gap-2.5 border-b border-line bg-page px-5 py-1.5">
        {params}
        <div className="ml-auto flex items-center gap-2.5">
          <PrimaryBtn onClick={() => void run()} disabled={busy}>
            {busy ? "生成中…" : "生成"}
          </PrimaryBtn>
        </div>
      </div>

      {kind === "text" ? (
        <div className="flex h-[38px] shrink-0 items-center gap-2 border-b border-line bg-page px-5">
          <span className="whitespace-nowrap text-[11.5px] text-muted">字符集</span>
          <input
            value={alphabet}
            onChange={(e) => setAlphabet(e.target.value)}
            spellCheck={false}
            placeholder="留空则用大小写字母 + 数字"
            className="min-w-0 flex-1 bg-transparent font-mono text-[12px] text-ink outline-none placeholder:font-sans placeholder:text-faint"
          />
          <span className="whitespace-nowrap text-[11px] text-faint">
            {alphabet.trim() ? `${new Set(Array.from(alphabet.trim())).size} 个` : "62 个"}
          </span>
        </div>
      ) : null}

      <div className="flex min-h-0 flex-1 gap-px bg-line">
        {kind === "pick" ? (
          <Pane
            label={
              <>
                候选清单
                <span className="font-normal text-faint">
                  {pool.split("\n").filter((l) => l.trim()).length} 条
                </span>
              </>
            }
          >
            <CodeEditor
              value={pool}
              onChange={setPool}
              language="text"
              placeholder="每行一条，抽到的顺序随机"
            />
          </Pane>
        ) : null}

        <Pane
          label={
            <>
              结果
              {stats ? <span className="font-normal text-faint">{stats}</span> : null}
            </>
          }
          actions={<CopyBtn text={out?.output ?? ""} />}
        >
          {err ? (
            <div className="p-4">
              <div className="rounded-[8px] border border-[#e6c3bb] bg-danger-soft p-3 text-[12px] leading-relaxed text-[#7d2e24]">
                {err}
              </div>
            </div>
          ) : (
            <CodeEditor value={out?.output ?? ""} language="text" readOnly placeholder="点「生成」" />
          )}
        </Pane>
      </div>
    </div>
  );
}
