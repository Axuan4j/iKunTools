import { useEffect, useMemo, useState } from "react";
import CodeEditor from "../components/CodeEditor";
import { CopyBtn, Pane, Segmented, TextBtn, Toggle } from "../components/ui";
import { IconBroom, IconChevron } from "../components/icons";
import { api, type JsonOutcome } from "../lib/api";

function fmtBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / 1024 / 1024).toFixed(2)} MB`;
}

function Scalar({ value }: { value: unknown }) {
  if (typeof value === "string")
    return (
      <span className="inline-block max-w-full truncate align-bottom text-[#8a5a00]">
        &quot;{value}&quot;
      </span>
    );
  if (typeof value === "number") return <span className="text-[#a13d63]">{String(value)}</span>;
  if (typeof value === "boolean") return <span className="text-[#7a3ea1]">{String(value)}</span>;
  return <span className="text-[#9a9792]">null</span>;
}

function TreeNode({
  k,
  value,
  depth,
  isIndex,
}: {
  k?: string;
  value: unknown;
  depth: number;
  isIndex?: boolean;
}) {
  const isArray = Array.isArray(value);
  const isObject = !isArray && value !== null && typeof value === "object";
  const isContainer = isArray || isObject;
  const [open, setOpen] = useState(depth < 2);
  const entries = isContainer ? Object.entries(value as Record<string, unknown>) : [];
  const count = isArray ? (value as unknown[]).length : entries.length;

  return (
    <div>
      <div className="flex items-start gap-1.5 rounded-[4px] px-1 hover:bg-[#f7f5f0]">
        <button
          onClick={() => isContainer && setOpen((o) => !o)}
          className={`mt-[6px] flex h-3 w-3 shrink-0 items-center justify-center rounded ${
            isContainer ? "text-faint hover:text-ink" : "invisible"
          }`}
        >
          <IconChevron className={`h-[9px] w-[9px] transition-transform ${open ? "rotate-90" : ""}`} />
        </button>
        {k !== undefined && (
          <>
            <span className={isIndex ? "text-faint" : "text-[#0b6e5f]"}>{k}</span>
            <span className="text-faint">:</span>
          </>
        )}
        {isContainer ? (
          <span className="text-faint">{isArray ? `[${count}]` : `{${count}}`}</span>
        ) : (
          <Scalar value={value} />
        )}
      </div>
      {isContainer && open && (
        <div className="ml-[13px] border-l border-line pl-2">
          {entries.map(([ck, cv], i) => (
            <TreeNode
              key={ck}
              k={isArray ? String(i) : ck}
              isIndex={isArray}
              value={cv}
              depth={depth + 1}
            />
          ))}
        </div>
      )}
    </div>
  );
}

export default function JsonTool() {
  const [input, setInput] = useState("");
  const [indent, setIndent] = useState<"2" | "4">("2");
  const [sortKeys, setSortKeys] = useState(false);
  const [mode, setMode] = useState<"pretty" | "minify">("pretty");
  const [tab, setTab] = useState<"text" | "tree">(() =>
    window.location.hash.endsWith("/tree") ? "tree" : "text",
  );
  const [result, setResult] = useState<JsonOutcome | null>(null);

  useEffect(() => {
    let alive = true;
    if (!input.trim()) {
      setResult(null);
      return;
    }
    const timer = setTimeout(() => {
      api
        .jsonFormat(input, Number(indent), sortKeys, mode === "minify")
        .then((r) => alive && setResult(r))
        .catch((e) =>
          alive &&
          setResult({
            ok: false,
            output: "",
            error: { message: String(e), line: 1, column: 1, snippet: "" },
            stats: { bytesIn: input.length, bytesOut: 0, nodes: 0, depth: 0, objects: 0, arrays: 0 },
            elapsedUs: 0,
          }),
        );
    }, 140);
    return () => {
      alive = false;
      clearTimeout(timer);
    };
  }, [input, indent, sortKeys, mode]);

  const treeValue = useMemo(() => {
    if (!result?.ok) return null;
    try {
      return JSON.parse(result.output) as unknown;
    } catch {
      return null;
    }
  }, [result]);

  const stats = result?.stats;

  return (
    <div className="flex h-full flex-col">
      <div className="flex h-[44px] shrink-0 items-center gap-3 border-b border-line bg-page px-5">
        <Segmented
          value={mode}
          onChange={setMode}
          options={[
            { value: "pretty", label: "格式化" },
            { value: "minify", label: "压缩" },
          ]}
        />
        <Segmented
          value={indent}
          onChange={setIndent}
          options={[
            { value: "2", label: "2 空格" },
            { value: "4", label: "4 空格" },
          ]}
        />
        <Toggle checked={sortKeys} onChange={setSortKeys}>
          键名排序
        </Toggle>
        <div className="ml-auto flex items-center gap-1.5">
          {stats ? (
            <span className="mr-1 font-mono text-[11px] text-faint">
              {fmtBytes(stats.bytesIn)} → {fmtBytes(stats.bytesOut)} · {stats.nodes} 节点 · 深度{" "}
              {stats.depth} · {Math.max(1, Math.round((result?.elapsedUs ?? 0) / 1000))} ms
            </span>
          ) : null}
        </div>
      </div>

      <div className="flex min-h-0 flex-1 gap-px bg-line">
        <Pane
          label={<>输入</>}
          actions={
            <>
              <TextBtn onClick={() => setInput("")} title="清空">
                <span className="flex items-center gap-1">
                  <IconBroom className="h-3 w-3" />
                  清空
                </span>
              </TextBtn>
            </>
          }
        >
          <CodeEditor value={input} onChange={setInput} language="json" placeholder="粘贴一段 JSON…" />
        </Pane>

        <Pane
          label={
            <>
              输出
              {result && !result.ok ? <span className="font-normal text-danger">解析失败</span> : null}
            </>
          }
          actions={
            <>
              <Segmented
                value={tab}
                onChange={setTab}
                options={[
                  { value: "text", label: "文本" },
                  { value: "tree", label: "树" },
                ]}
              />
              <CopyBtn text={result?.ok ? result.output : ""} />
            </>
          }
        >
          {result && !result.ok ? (
            <div className="p-4">
              <div className="rounded-[8px] border border-[#e6c3bb] bg-danger-soft p-3">
                <div className="flex items-baseline gap-2">
                  <span className="text-[12px] font-medium text-danger">
                    第 {result.error?.line} 行 · 第 {result.error?.column} 列
                  </span>
                  <span className="text-[11px] text-danger/70">语法错误</span>
                </div>
                <p className="mt-1.5 text-[12px] leading-relaxed text-[#7d2e24]">
                  {result.error?.message}
                </p>
                {result.error?.snippet ? (
                  <pre className="mt-2 overflow-x-auto rounded-[5px] bg-white/60 px-2 py-1.5 font-mono text-[11.5px] leading-relaxed text-ink/60">
                    {result.error.snippet}
                  </pre>
                ) : null}
              </div>
            </div>
          ) : tab === "tree" ? (
            <div className="h-full overflow-auto px-3 py-2 font-mono text-[12.5px] leading-[1.75]">
              {treeValue !== null ? <TreeNode value={treeValue} depth={0} /> : null}
            </div>
          ) : (
            <CodeEditor value={result?.output ?? ""} language="json" readOnly />
          )}
        </Pane>
      </div>
    </div>
  );
}
