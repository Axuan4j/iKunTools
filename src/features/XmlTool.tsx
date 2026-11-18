import { useEffect, useState } from "react";
import CodeEditor from "../components/CodeEditor";
import { CopyBtn, Pane, Segmented, Select, Toggle } from "../components/ui";
import { api, type XmlOutcome, type XmlSpec } from "../lib/api";

type Mode = "jsonToXml" | "xmlToJson" | "format" | "minify";

const DEFAULT_SPEC: XmlSpec = {
  indent: 2,
  declaration: true,
  root: "root",
  attrPrefix: "@",
  textKey: "#text",
  trim: true,
  numbers: true,
  keepComments: true,
};

const MODES: { value: Mode; label: string }[] = [
  { value: "jsonToXml", label: "JSON → XML" },
  { value: "xmlToJson", label: "XML → JSON" },
  { value: "format", label: "格式化" },
  { value: "minify", label: "压缩" },
];

const INDENTS = [
  { value: "2", label: "缩进 2" },
  { value: "4", label: "缩进 4" },
  { value: "0", label: "单行" },
];

function MiniInput({
  label,
  value,
  onChange,
  width,
  placeholder,
}: {
  label: string;
  value: string;
  onChange: (v: string) => void;
  width: number;
  placeholder?: string;
}) {
  return (
    <label className="flex shrink-0 items-center gap-1.5">
      <span className="shrink-0 text-[11px] text-faint">{label}</span>
      <input
        value={value}
        onChange={(e) => onChange(e.target.value)}
        spellCheck={false}
        autoCapitalize="off"
        autoCorrect="off"
        placeholder={placeholder}
        style={{ width }}
        className="h-[26px] rounded-[6px] border border-line bg-panel px-2 font-mono text-[11.5px] text-ink outline-none placeholder:text-faint focus:border-line-strong"
      />
    </label>
  );
}

const ms = (us: number) => `${(us / 1000).toFixed(us < 10000 ? 1 : 0)} ms`;

export default function XmlTool() {
  const [mode, setMode] = useState<Mode>("jsonToXml");
  const [spec, setSpec] = useState<XmlSpec>(DEFAULT_SPEC);
  const [input, setInput] = useState("");
  const [out, setOut] = useState<XmlOutcome | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (!input.trim()) {
      setOut(null);
      setErr(null);
      setBusy(false);
      return;
    }
    let alive = true;
    setBusy(true);
    const timer = setTimeout(() => {
      const job =
        mode === "jsonToXml"
          ? api.xmlJsonToXml(spec, input)
          : mode === "xmlToJson"
            ? api.xmlXmlToJson(spec, input)
            : mode === "format"
              ? api.xmlFormat(spec, input)
              : api.xmlMinify(spec, input);
      job
        .then((r) => {
          if (alive) {
            setOut(r);
            setErr(null);
          }
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
  }, [mode, spec, input]);

  const inputBytes = new TextEncoder().encode(input).length;
  const showErr = !busy && err;

  const status = busy
    ? "计算中…"
    : out
      ? [
          `${out.elements} 元素`,
          `深度 ${out.depth}`,
          out.attributes > 0 ? `${out.attributes} 属性` : null,
          out.comments > 0 ? `${out.comments} 注释` : null,
          `${out.outputBytes} B`,
          out.renamed > 0 ? `${out.renamed} 名称已修正` : null,
          ms(out.elapsedUs),
        ]
          .filter(Boolean)
          .join(" · ")
      : null;

  const outLabel =
    mode === "jsonToXml"
      ? "XML"
      : mode === "xmlToJson"
        ? "JSON"
        : mode === "format"
          ? "格式化结果"
          : "压缩结果";

  const inputLang: "json" | "text" = mode === "jsonToXml" ? "json" : "text";
  const outputLang: "json" | "text" = mode === "xmlToJson" ? "json" : "text";

  return (
    <div className="flex h-full flex-col">
      <div className="flex min-h-[44px] shrink-0 flex-wrap items-center gap-x-3 gap-y-2 border-b border-line bg-page px-5 py-2">
        <Segmented value={mode} onChange={setMode} options={MODES} />
        <span className="h-[15px] w-px shrink-0 bg-line" />

        {mode === "jsonToXml" ? (
          <>
            <MiniInput
              label="根元素"
              value={spec.root}
              onChange={(v) => setSpec({ ...spec, root: v })}
              width={80}
              placeholder="root"
            />
            <Select
              value={String(spec.indent)}
              onChange={(v) => setSpec({ ...spec, indent: Number(v) })}
              options={INDENTS}
              title="缩进宽度"
            />
            <Toggle checked={spec.declaration} onChange={(v) => setSpec({ ...spec, declaration: v })}>
              声明
            </Toggle>
          </>
        ) : null}

        {mode === "xmlToJson" ? (
          <>
            <MiniInput
              label="属性前缀"
              value={spec.attrPrefix}
              onChange={(v) => setSpec({ ...spec, attrPrefix: v })}
              width={44}
              placeholder="留空关闭"
            />
            <MiniInput
              label="文本键"
              value={spec.textKey}
              onChange={(v) => setSpec({ ...spec, textKey: v })}
              width={64}
              placeholder="#text"
            />
            <Select
              value={String(spec.indent)}
              onChange={(v) => setSpec({ ...spec, indent: Number(v) })}
              options={INDENTS}
              title="JSON 缩进宽度"
            />
            <Toggle checked={spec.numbers} onChange={(v) => setSpec({ ...spec, numbers: v })}>
              数字/布尔
            </Toggle>
          </>
        ) : null}

        {mode === "format" ? (
          <>
            <Select
              value={String(spec.indent)}
              onChange={(v) => setSpec({ ...spec, indent: Number(v) })}
              options={INDENTS}
              title="缩进宽度"
            />
            <Toggle checked={spec.declaration} onChange={(v) => setSpec({ ...spec, declaration: v })}>
              声明
            </Toggle>
            <Toggle
              checked={spec.keepComments}
              onChange={(v) => setSpec({ ...spec, keepComments: v })}
            >
              注释
            </Toggle>
          </>
        ) : null}

        {mode === "minify" ? (
          <>
            <Toggle
              checked={spec.keepComments}
              onChange={(v) => setSpec({ ...spec, keepComments: v })}
            >
              注释
            </Toggle>
            <Toggle checked={spec.declaration} onChange={(v) => setSpec({ ...spec, declaration: v })}>
              声明
            </Toggle>
          </>
        ) : null}
      </div>

      <div className="flex min-h-0 flex-1 gap-px bg-line">
        <Pane
          grow={mode === "xmlToJson" || mode === "format" ? 1 : 1.25}
          label={
            <>
              输入
              <span className="font-normal text-faint">{inputBytes} B</span>
            </>
          }
        >
          <CodeEditor
            value={input}
            onChange={setInput}
            language={inputLang}
            placeholder={mode === "jsonToXml" ? "粘贴 JSON…" : "粘贴 XML…"}
          />
        </Pane>

        <Pane
          label={
            <>
              {outLabel}
              {status ? (
                <span className="whitespace-nowrap font-normal text-faint">{status}</span>
              ) : null}
            </>
          }
          actions={out ? <CopyBtn text={out.output} /> : null}
        >
          {showErr ? (
            <div className="p-4">
              <div className="rounded-[8px] border border-[#e6c3bb] bg-danger-soft p-3 text-[12px] leading-relaxed text-[#7d2e24]">
                {showErr}
              </div>
            </div>
          ) : !input.trim() ? (
            <div className="flex h-full items-center justify-center text-[12px] text-faint">
              等待输入
            </div>
          ) : (
            <CodeEditor
              value={out?.output ?? ""}
              language={outputLang}
              readOnly
              placeholder="结果…"
            />
          )}
        </Pane>
      </div>
    </div>
  );
}
