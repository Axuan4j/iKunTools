import { useEffect, useState } from "react";
import CodeEditor from "../components/CodeEditor";
import { CopyBtn, Pane, Segmented, Toggle } from "../components/ui";
import { IconBroom } from "../components/icons";
import { api, type UrlKeep, type UrlOutcome, type UrlSpec } from "../lib/api";

type Mode = "encode" | "decode";

const KEEPS: { value: UrlKeep; label: string }[] = [
  { value: "component", label: "组件" },
  { value: "uri", label: "完整 URL" },
];

export default function UrlTool() {
  const [mode, setMode] = useState<Mode>("encode");
  const [keep, setKeep] = useState<UrlKeep>("component");
  const [plus, setPlus] = useState(false);
  const [upper, setUpper] = useState(true);
  const [input, setInput] = useState("");
  const [out, setOut] = useState<UrlOutcome | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let alive = true;
    if (!input.trim()) {
      setOut(null);
      setErr(null);
      return;
    }
    setBusy(true);
    const timer = setTimeout(() => {
      const spec: UrlSpec = { keep, plusForSpace: plus, upperHex: upper };
      const job = mode === "encode" ? api.urlEncode(spec, input) : api.urlDecode(spec, input);
      job
        .then((r) => {
          if (!alive) return;
          setOut(r);
          setErr(null);
        })
        .catch((e) => {
          if (!alive) return;
          setOut(null);
          setErr(String(e));
        })
        .finally(() => alive && setBusy(false));
    }, 120);
    return () => {
      alive = false;
      clearTimeout(timer);
    };
  }, [mode, keep, plus, upper, input]);

  const inputBytes = new TextEncoder().encode(input).length;

  return (
    <div className="flex h-full flex-col">
      <div className="flex h-[44px] shrink-0 items-center gap-3 border-b border-line bg-page px-5">
        <Segmented
          value={mode}
          onChange={setMode}
          options={[
            { value: "encode", label: "编码" },
            { value: "decode", label: "解码" },
          ]}
        />
        <span className="h-[15px] w-px bg-line" />
        {mode === "encode" ? (
          <>
            <Segmented value={keep} onChange={setKeep} options={KEEPS} />
            <Toggle checked={plus} onChange={setPlus}>
              空格 → +
            </Toggle>
            <Toggle checked={upper} onChange={setUpper}>
              大写
            </Toggle>
          </>
        ) : (
          <Toggle checked={plus} onChange={setPlus}>
            + 当空格
          </Toggle>
        )}
      </div>

      <div className="flex min-h-0 flex-1 gap-px bg-line">
        <Pane
          label={
            <>
              {mode === "encode" ? "原文" : "编码文本"}
              <span className="font-normal text-faint">{inputBytes} B</span>
            </>
          }
          actions={
            <button
              onClick={() => setInput("")}
              className="flex items-center gap-1 rounded-[6px] px-2 py-1 text-[12px] text-muted hover:bg-[#f2f0eb] hover:text-ink"
            >
              <IconBroom className="h-3 w-3" />
              清空
            </button>
          }
        >
          <CodeEditor
            value={input}
            onChange={setInput}
            language="text"
            placeholder={mode === "encode" ? "要编码的文本或 URL…" : "含 %XX 的文本…"}
          />
        </Pane>

        <Pane
          label={
            <>
              {mode === "encode" ? "编码结果" : "原文"}
              {busy ? (
                <span className="font-normal text-faint">计算中…</span>
              ) : out ? (
                <span className="font-normal text-faint">
                  {out.escapes} 个 %XX · {out.outputBytes} B
                </span>
              ) : null}
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
            <CodeEditor value={out?.output ?? ""} language="text" readOnly placeholder="结果…" />
          )}
        </Pane>
      </div>
    </div>
  );
}
