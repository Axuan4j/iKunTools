import { useEffect, useMemo, useRef, useState } from "react";
import type { ToolDef } from "../features/registry";
import { IconChevron } from "./icons";
import { Kbd } from "./ui";

export default function CommandPalette({
  open,
  tools,
  onClose,
  onPick,
}: {
  open: boolean;
  tools: ToolDef[];
  onClose: () => void;
  onPick: (id: string) => void;
}) {
  const [q, setQ] = useState("");
  const [idx, setIdx] = useState(0);
  const listRef = useRef<HTMLUListElement | null>(null);

  useEffect(() => {
    if (open) {
      setQ("");
      setIdx(0);
    }
  }, [open]);

  const results = useMemo(() => {
    const key = q.trim().toLowerCase();
    if (!key) return tools;
    const scored = tools
      .map((t) => {
        const haystack = `${t.name} ${t.desc} ${t.keywords.join(" ")} ${t.group}`.toLowerCase();
        let score = 0;
        if (t.name.toLowerCase().includes(key)) score += 100;
        if (t.keywords.some((k) => k.toLowerCase().startsWith(key))) score += 50;
        if (haystack.includes(key)) score += 10;
        return { t, score };
      })
      .filter((x) => x.score > 0);
    return scored.sort((a, b) => b.score - a.score).map((x) => x.t);
  }, [q, tools]);

  useEffect(() => setIdx(0), [q]);

  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        onClose();
      } else if (e.key === "ArrowDown") {
        e.preventDefault();
        setIdx((i) => (results.length ? (i + 1) % results.length : 0));
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        setIdx((i) => (results.length ? (i - 1 + results.length) % results.length : 0));
      } else if (e.key === "Enter") {
        e.preventDefault();
        const picked = results[idx];
        if (picked) {
          onPick(picked.id);
          onClose();
        }
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, results, idx, onClose, onPick]);

  useEffect(() => {
    const el = listRef.current?.querySelector<HTMLElement>(`[data-i="${idx}"]`);
    el?.scrollIntoView({ block: "nearest" });
  }, [idx]);

  if (!open) return null;

  return (
    <div
      className="fixed inset-0 z-50 flex items-start justify-center bg-[rgba(26,26,24,0.06)] pt-[14vh]"
      onMouseDown={onClose}
    >
      <div
        className="w-[540px] overflow-hidden rounded-[12px] border border-line bg-panel shadow-[0_18px_50px_rgba(23,23,20,0.16)]"
        onMouseDown={(e) => e.stopPropagation()}
      >
        <div className="flex items-center gap-2 border-b border-line px-3.5 py-2.5">
          <svg viewBox="0 0 16 16" className="h-4 w-4 text-faint" fill="none" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round">
            <circle cx="7.2" cy="7.2" r="4.2" />
            <path d="M10.4 10.4 13.4 13.4" />
          </svg>
          <input
            autoFocus
            value={q}
            onChange={(e) => setQ(e.target.value)}
            placeholder="搜索工具，输入 json / 哈希 / jwt…"
            className="w-full bg-transparent text-[13px] placeholder:text-faint"
          />
          <Kbd>esc</Kbd>
        </div>

        <ul ref={listRef} className="max-h-[330px] overflow-auto p-1.5">
          {results.length === 0 ? (
            <li className="px-3 py-8 text-center text-[12.5px] text-faint">没有匹配的工具</li>
          ) : (
            results.map((t, i) => {
              const active = i === idx;
              const Icon = t.icon;
              return (
                <li key={t.id} data-i={i}>
                  <button
                    onMouseEnter={() => setIdx(i)}
                    onClick={() => {
                      onPick(t.id);
                      onClose();
                    }}
                    className={`flex w-full items-center gap-2.5 rounded-[7px] px-2.5 py-[7px] text-left ${
                      active ? "bg-accent-soft" : ""
                    }`}
                  >
                    <Icon className={`h-[15px] w-[15px] shrink-0 ${active ? "text-accent" : "text-faint"}`} />
                    <span className="min-w-0 flex-1">
                      <span className="block truncate text-[12.5px] text-ink">{t.name}</span>
                      <span className="block truncate text-[11px] text-faint">{t.desc}</span>
                    </span>
                    <span className="shrink-0 text-[10.5px] text-faint">{t.group}</span>
                    {active ? <IconChevron className="h-3 w-3 shrink-0 text-accent" /> : null}
                  </button>
                </li>
              );
            })
          )}
        </ul>

        <div className="flex items-center gap-3 border-t border-line px-3.5 py-1.5 text-[10.5px] text-faint">
          <span className="flex items-center gap-1">
            <Kbd>↑</Kbd>
            <Kbd>↓</Kbd> 选择
          </span>
          <span className="flex items-center gap-1">
            <Kbd>↵</Kbd> 打开
          </span>
          <span className="ml-auto">{results.length} 个工具</span>
        </div>
      </div>
    </div>
  );
}
