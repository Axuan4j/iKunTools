import { useEffect, useState } from "react";
import CodeEditor from "../components/CodeEditor";
import { CopyBtn, Pane, Segmented, Toggle } from "../components/ui";
import { IconBroom } from "../components/icons";
import { api, type AsciiOutcome, type AsciiSpec, type AsciiStyle } from "../lib/api";

type Mode = "encode" | "decode";

const STYLES: { value: AsciiStyle; label: string }[] = [
  { value: "js", label: "\\uXXXX" },
  { value: "htmlHex", label: "&#xXXXX;" },
  { value: "codePoint", label: "U+XXXX" },
];

export default function AsciiTool() {
  const [mode, setMode] = useState<Mode>("encode");
  const [style, setStyle] = useState<AsciiStyle>("js");
  const [upper, setUpper] = useState(false);
  const [input, setInput] = useState("");
  const [out, setOut] = useState<AsciiOutcome | null>(null);
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
      const spec: AsciiSpec = { style, upperHex: upper };
      const job = mode === "encode" ? api.asciiEncode(spec, input) : api.asciiDecode(input);
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
  }, [mode, style, upper, input]);

  const inputBytes = new TextEncoder().encode(input).length;
  const status = busy
    ? "计算中…"
    : out
      ? mode === "encode"
        ? `${out.converted} 个字符被转义 · ${out.outputBytes} B`
        : `${out.converted} 个转义已还原 · ${out.outputBytes} B`
      : null;

  return (
    <div className="flex h-full flex-col">
      <div className="flex h-[44px] shrink-0 items-center gap-3 border-b border-line bg-page px-5">
        <Segmented
          value={mode}
          onChange={setMode}
          options={[
            { value: "encode", label: "转 ASCII" },
            { value: "decode", label: "还原文本" },
          ]}
        />
        {mode === "encode" ? (
          <>
            <span className="h-[15px] w-px bg-line" />
            <Segmented value={style} onChange={setStyle} options={STYLES} />
            <Toggle checked={upper} onChange={setUpper}>
              大写
            </Toggle>
          </>
        ) : null}
      </div>

      <div className="flex min-h-0 flex-1 gap-px bg-line">
        <Pane
          label={
            <>
              {mode === "encode" ? "原文" : "ASCII 转义"}
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
            placeholder={mode === "encode" ? "要转成 ASCII 的文本…" : "\\u4e2d / &#x4e2d; / U+4E2D…"}
          />
        </Pane>

        <Pane
          label={
            <>
              {mode === "encode" ? "ASCII 转义" : "原文"}
              {status ? <span className="font-normal text-faint">{status}</span> : null}
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
