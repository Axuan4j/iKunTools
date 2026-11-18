import { useEffect, useState } from "react";
import CodeEditor from "../components/CodeEditor";
import { Chip, CopyBtn, Pane, Toggle } from "../components/ui";
import { IconBroom, IconKey } from "../components/icons";
import { api, type HashOutcome } from "../lib/api";

const ALGS = [
  { id: "md5", label: "MD5" },
  { id: "sha1", label: "SHA-1" },
  { id: "sha256", label: "SHA-256" },
  { id: "sha512", label: "SHA-512" },
  { id: "blake3", label: "BLAKE3" },
  { id: "hmac-sha256", label: "HMAC-SHA256" },
];

export default function HashTool() {
  const [alg, setAlg] = useState("sha256");
  const [input, setInput] = useState("");
  const [key, setKey] = useState("");
  const [upper, setUpper] = useState(false);
  const [out, setOut] = useState<HashOutcome | null>(null);
  const [err, setErr] = useState<string | null>(null);

  useEffect(() => {
    let alive = true;
    if (!input) {
      setOut(null);
      setErr(null);
      return;
    }
    const timer = setTimeout(() => {
      api
        .digest(alg, input, alg === "hmac-sha256" ? key : undefined)
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
  }, [alg, input, key]);

  const hex = out ? (upper ? out.hex.toUpperCase() : out.hex) : "";

  return (
    <div className="flex h-full flex-col">
      <div className="flex h-[44px] shrink-0 items-center gap-3 border-b border-line bg-page px-5">
        <div className="flex items-center gap-1">
          {ALGS.map((a) => (
            <Chip key={a.id} active={alg === a.id} onClick={() => setAlg(a.id)}>
              {a.label}
            </Chip>
          ))}
        </div>

        {alg === "hmac-sha256" ? (
          <label className="flex items-center gap-1.5 rounded-[7px] border border-line bg-panel px-2 py-[4px]">
            <IconKey className="h-[13px] w-[13px] text-faint" />
            <input
              value={key}
              onChange={(e) => setKey(e.target.value)}
              placeholder="密钥"
              className="w-[120px] bg-transparent font-mono text-[11.5px] placeholder:text-faint"
            />
          </label>
        ) : null}

        <div className="ml-auto">
          <Toggle checked={upper} onChange={setUpper}>
            大写
          </Toggle>
        </div>
      </div>

      <div className="flex min-h-0 flex-1 gap-px bg-line">
        <Pane
          label={<>输入</>}
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
            placeholder="要计算摘要的文本…"
          />
        </Pane>

        <Pane
          label={
            <>
              摘要
              {out ? (
                <span className="font-normal text-faint">
                  {out.algorithm.toUpperCase()} · {out.bits} bit
                </span>
              ) : null}
            </>
          }
          actions={<CopyBtn text={hex} />}
        >
          <div className="h-full overflow-auto p-4">
            {err ? (
              <div className="rounded-[8px] border border-[#e6c3bb] bg-danger-soft p-3 text-[12px] text-[#7d2e24]">
                {err}
              </div>
            ) : !out ? null : (
              <div className="space-y-4">
                <div>
                  <div className="mb-1.5 flex items-center justify-between">
                    <span className="text-[10.5px] font-medium tracking-[0.04em] text-faint">HEX</span>
                    <CopyBtn text={hex} label="复制 hex" />
                  </div>
                  <div className="rounded-[7px] border border-line bg-[#faf9f6] p-3 font-mono text-[12.5px] leading-[1.8] break-all text-ink select-all">
                    {hex || <span className="text-faint">—</span>}
                  </div>
                </div>

                <div>
                  <div className="mb-1.5 flex items-center justify-between">
                    <span className="text-[10.5px] font-medium tracking-[0.04em] text-faint">BASE64</span>
                    <CopyBtn text={out.base64} label="复制 base64" />
                  </div>
                  <div className="rounded-[7px] border border-line bg-[#faf9f6] p-3 font-mono text-[12px] leading-[1.8] break-all text-muted select-all">
                    {out.base64}
                  </div>
                </div>

                <p className="font-mono text-[11px] text-faint">
                  {new TextEncoder().encode(input).length} B → {out.bytes} B
                </p>
              </div>
            )}
          </div>
        </Pane>
      </div>
    </div>
  );
}
