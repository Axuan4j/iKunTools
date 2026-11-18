import { useEffect, useState } from "react";
import CodeEditor from "../components/CodeEditor";
import { Chip, CopyBtn, Pane, Segmented } from "../components/ui";
import { IconBroom } from "../components/icons";
import { api, type RegexOutcome, type RegexSpec, type ReplaceOutcome } from "../lib/api";

type Mode = "test" | "replace";

const FLAGS: { key: keyof RegexSpec; label: string; title: string }[] = [
  { key: "ignoreCase", label: "i", title: "忽略大小写" },
  { key: "multiLine", label: "m", title: "多行：^ $ 匹配每行首尾" },
  { key: "dotAll", label: "s", title: "单行：. 也匹配换行" },
  { key: "extended", label: "x", title: "忽略表达式里的空白" },
  { key: "unicode", label: "u", title: "Unicode：关掉后 \\w \\d \\b 只认 ASCII" },
];

const DEFAULT_SPEC: RegexSpec = {
  ignoreCase: false,
  multiLine: false,
  dotAll: false,
  extended: false,
  unicode: true,
};

const ms = (us: number) => `${(us / 1000).toFixed(us < 10000 ? 1 : 0)} ms`;

export default function RegexTool() {
  const [mode, setMode] = useState<Mode>("test");
  const [spec, setSpec] = useState<RegexSpec>(DEFAULT_SPEC);
  const [pattern, setPattern] = useState("");
  const [text, setText] = useState("");
  const [replacement, setReplacement] = useState("");
  const [matches, setMatches] = useState<RegexOutcome | null>(null);
  const [repl, setRepl] = useState<ReplaceOutcome | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let alive = true;
    if (!pattern.trim()) {
      setMatches(null);
      setRepl(null);
      setErr(null);
      return;
    }
    setBusy(true);
    const timer = setTimeout(() => {
      const job =
        mode === "test"
          ? api.regexTest(spec, pattern, text).then((r) => {
              if (alive) {
                setMatches(r);
                setErr(null);
              }
            })
          : api.regexReplace(spec, pattern, text, replacement).then((r) => {
              if (alive) {
                setRepl(r);
                setErr(null);
              }
            });
      job
        .catch((e) => {
          if (!alive) return;
          setMatches(null);
          setRepl(null);
          setErr(String(e));
        })
        .finally(() => alive && setBusy(false));
    }, 120);
    return () => {
      alive = false;
      clearTimeout(timer);
    };
  }, [mode, spec, pattern, text, replacement]);

  const textBytes = new TextEncoder().encode(text).length;
  const flagStr = FLAGS.filter((f) => spec[f.key]).map((f) => f.label).join("") || "—";
  const showErr = !busy && err;

  const status =
    mode === "test"
      ? busy
        ? "计算中…"
        : matches
          ? `${matches.count} 处匹配${matches.truncated ? " · 仅列前 2000" : ""} · ${ms(matches.elapsedUs)}`
          : null
      : busy
        ? "计算中…"
        : repl
          ? `${repl.replacements} 处替换 · ${repl.outputBytes} B · ${ms(repl.elapsedUs)}`
          : null;

  return (
    <div className="flex h-full flex-col">
      <div className="flex h-[44px] shrink-0 items-center gap-3 border-b border-line bg-page px-5">
        <Segmented
          value={mode}
          onChange={setMode}
          options={[
            { value: "test", label: "测试" },
            { value: "replace", label: "替换" },
          ]}
        />
        <span className="h-[15px] w-px bg-line" />
        <div className="flex items-center gap-1">
          {FLAGS.map((f) => (
            <span key={f.key} title={f.title}>
              <Chip
                active={spec[f.key]}
                onClick={() => setSpec({ ...spec, [f.key]: !spec[f.key] })}
              >
                {f.label}
              </Chip>
            </span>
          ))}
        </div>
      </div>

      <div className="flex min-h-0 flex-1 gap-px bg-line">
        <Pane
          grow={1.7}
          label={
            <>
              文本
              <span className="font-normal text-faint">{textBytes} B</span>
            </>
          }
          actions={
            <button
              onClick={() => setText("")}
              className="flex items-center gap-1 rounded-[6px] px-2 py-1 text-[12px] text-muted hover:bg-[#f2f0eb] hover:text-ink"
            >
              <IconBroom className="h-3 w-3" />
              清空
            </button>
          }
        >
          <div className="flex h-full min-h-0 flex-col">
            <div className="flex h-[50px] shrink-0 items-center gap-2 border-b border-line px-3.5">
              <span className="shrink-0 font-mono text-[13px] text-faint">/</span>
              <input
                value={pattern}
                onChange={(e) => setPattern(e.target.value)}
                spellCheck={false}
                autoCapitalize="off"
                autoCorrect="off"
                placeholder="正则表达式…"
                className="min-w-0 flex-1 bg-transparent font-mono text-[13.5px] text-ink outline-none placeholder:text-faint"
              />
              <span className="shrink-0 font-mono text-[11.5px] text-faint">/{flagStr}</span>
            </div>

            {mode === "replace" ? (
              <div className="flex h-[44px] shrink-0 items-center gap-2 border-b border-line px-3.5">
                <span className="shrink-0 text-[11px] text-faint">替换为</span>
                <input
                  value={replacement}
                  onChange={(e) => setReplacement(e.target.value)}
                  spellCheck={false}
                  autoCapitalize="off"
                  autoCorrect="off"
                  placeholder="$1 / $name / $$"
                  className="min-w-0 flex-1 bg-transparent font-mono text-[13.5px] text-ink outline-none placeholder:text-faint"
                />
              </div>
            ) : null}

            <div className="min-h-0 flex-1">
              <CodeEditor value={text} onChange={setText} language="text" placeholder="待匹配的文本…" />
            </div>
          </div>
        </Pane>

        <Pane
          label={
            <>
              {mode === "test" ? "匹配" : "替换结果"}
              {status ? (
                <span className="whitespace-nowrap font-normal text-faint">{status}</span>
              ) : null}
            </>
          }
          actions={mode === "replace" ? <CopyBtn text={repl?.output ?? ""} /> : null}
        >
          {showErr ? (
            <div className="p-4">
              <div className="rounded-[8px] border border-[#e6c3bb] bg-danger-soft p-3 text-[12px] leading-relaxed text-[#7d2e24]">
                {showErr}
              </div>
            </div>
          ) : mode === "replace" ? (
            <CodeEditor value={repl?.output ?? ""} language="text" readOnly placeholder="结果…" />
          ) : !matches ? (
            <div className="flex h-full items-center justify-center text-[12px] text-faint">
              {pattern.trim() ? "" : "等待表达式"}
            </div>
          ) : matches.count === 0 ? (
            <div className="flex h-full items-center justify-center text-[12px] text-faint">
              无匹配
            </div>
          ) : (
            <div className="h-full overflow-y-auto">
              {matches.matches.map((m) => (
                <div key={m.index} className="border-b border-line px-3 py-2">
                  <div className="flex flex-wrap items-center gap-x-2 gap-y-1 font-mono text-[11px] text-faint">
                    <span className="text-accent">#{m.index}</span>
                    <span className="whitespace-nowrap">
                      L{m.line}:{m.column}
                    </span>
                    <span className="whitespace-nowrap">
                      [{m.start}..{m.end}]
                    </span>
                    <span className="ml-auto whitespace-nowrap">{m.end - m.start} B</span>
                  </div>
                  <div className="mt-1 max-h-[120px] overflow-hidden whitespace-pre-wrap break-all rounded-[5px] bg-[#f7f5f1] px-2 py-1 font-mono text-[12px] leading-[1.6] text-ink">
                    {m.text || <span className="text-faint">(空匹配)</span>}
                  </div>
                  {m.groups.length > 0 ? (
                    <div className="mt-1 space-y-[1px]">
                      {m.groups.map((g) => (
                        <div key={g.index} className="flex gap-2 pl-1 text-[11.5px] leading-[1.6]">
                          <span className="w-[64px] shrink-0 truncate font-mono text-faint">
                            {g.name ? `$${g.name}` : `$${g.index}`}
                          </span>
                          <span
                            className={`min-w-0 flex-1 break-all font-mono ${
                              g.text === null ? "text-faint" : "text-muted"
                            }`}
                          >
                            {g.text === null ? "未参与" : g.text || "(空)"}
                          </span>
                        </div>
                      ))}
                    </div>
                  ) : null}
                </div>
              ))}
            </div>
          )}
        </Pane>
      </div>
    </div>
  );
}
