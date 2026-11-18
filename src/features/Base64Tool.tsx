import { useEffect, useState } from "react";
import CodeEditor from "../components/CodeEditor";
import { CopyBtn, Pane, Segmented, Toggle } from "../components/ui";
import { api } from "../lib/api";

type Mode = "encode" | "decode";

export default function Base64Tool() {
  const [mode, setMode] = useState<Mode>("encode");
  const [urlSafe, setUrlSafe] = useState(false);
  const [noPad, setNoPad] = useState(false);
  const [input, setInput] = useState("");
  const [out, setOut] = useState("");
  const [err, setErr] = useState<string | null>(null);

  useEffect(() => {
    let alive = true;
    if (!input.trim()) {
      setOut("");
      setErr(null);
      return;
    }
    const timer = setTimeout(() => {
      const job =
        mode === "encode"
          ? api.base64Encode(input, urlSafe, noPad)
          : api.base64Decode(input);
      job
        .then((r) => {
          if (!alive) return;
          setOut(r);
          setErr(null);
        })
        .catch((e) => {
          if (!alive) return;
          setOut("");
          setErr(String(e));
        });
    }, 120);
    return () => {
      alive = false;
      clearTimeout(timer);
    };
  }, [mode, input, urlSafe, noPad]);

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
        <Toggle checked={urlSafe} onChange={setUrlSafe}>
          URL 安全字符
        </Toggle>
        <Toggle checked={noPad} onChange={setNoPad}>
          去掉 padding
        </Toggle>
      </div>

      <div className="flex min-h-0 flex-1 gap-px bg-line">
        <Pane
          label={
            <>
              {mode === "encode" ? "原文" : "Base64"}
              {mode === "encode" ? (
                <span className="font-normal text-faint">{new TextEncoder().encode(input).length} 字节</span>
              ) : null}
            </>
          }
        >
          <CodeEditor value={input} onChange={setInput} language="text" placeholder="输入内容…" />
        </Pane>

        <Pane
          label={
            <>
              {mode === "encode" ? "Base64" : "原文"}
              {out ? <span className="font-normal text-faint">{out.length} 字符</span> : null}
            </>
          }
          actions={<CopyBtn text={out} />}
        >
          {err ? (
            <div className="p-4">
              <div className="rounded-[8px] border border-[#e6c3bb] bg-danger-soft p-3 text-[12px] leading-relaxed text-[#7d2e24]">
                {err}
              </div>
            </div>
          ) : (
            <CodeEditor value={out} language="text" readOnly placeholder="结果…" />
          )}
        </Pane>
      </div>
    </div>
  );
}
