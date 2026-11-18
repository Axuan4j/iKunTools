import { TOOLS } from "../features/registry";
import { IconSearch } from "../components/icons";
import { Kbd } from "../components/ui";
import { inTauri } from "../lib/api";

export default function Sidebar({
  activeId,
  onSelect,
  onOpenPalette,
}: {
  activeId: string;
  onSelect: (id: string) => void;
  onOpenPalette: () => void;
}) {
  const groups = Array.from(new Set(TOOLS.map((t) => t.group)));

  return (
    <aside className="flex w-[228px] shrink-0 flex-col border-r border-line bg-side">
      <div data-tauri-drag-region className="flex h-[56px] shrink-0 items-center pl-[84px]">
        <span className="select-none text-[12.5px] font-medium tracking-[0.01em] text-ink">iKunTools</span>
      </div>

      <div className="px-2.5 pb-2">
        <button
          onClick={onOpenPalette}
          className="flex w-full items-center gap-1.5 rounded-[7px] border border-line bg-panel px-2 py-[5px] text-[11.5px] text-faint transition-colors hover:border-line-strong hover:text-muted"
        >
          <IconSearch className="h-[13px] w-[13px]" />
          搜索工具
          <span className="ml-auto">
            <Kbd>⌘K</Kbd>
          </span>
        </button>
      </div>

      <nav className="min-h-0 flex-1 overflow-y-auto px-2 pb-3">
        {groups.map((g) => (
          <div key={g} className="mb-0.5">
            <div className="px-2 pb-1 pt-2 text-[10.5px] font-medium tracking-[0.04em] text-faint">{g}</div>
            {TOOLS.filter((t) => t.group === g).map((t) => {
              const active = t.id === activeId;
              const Icon = t.icon;
              return (
                <button
                  key={t.id}
                  onClick={() => onSelect(t.id)}
                  className={`mb-[1px] flex w-full items-center gap-2 rounded-[6px] px-2 py-[5px] text-left transition-colors ${
                    active
                      ? "bg-panel text-ink shadow-[0_1px_2px_rgba(23,23,20,0.05)] ring-1 ring-line"
                      : "text-muted hover:bg-[#efede7] hover:text-ink"
                  }`}
                >
                  <Icon className={`h-[14px] w-[14px] shrink-0 ${active ? "text-accent" : "text-faint"}`} />
                  <span className="truncate text-[12.5px]">{t.name}</span>
                </button>
              );
            })}
          </div>
        ))}
      </nav>

      <div className="shrink-0 border-t border-line px-3 py-2">
        <div className="flex items-center justify-between text-[10.5px] text-faint">
          <span>v0.1.0 · M1</span>
          <span
            className="flex items-center gap-1"
            title={inTauri() ? "Rust 本地执行" : "浏览器预览（WebCrypto 兜底）"}
          >
            <span className={`h-[4px] w-[4px] rounded-full ${inTauri() ? "bg-accent" : "bg-warn"}`} />
            本地
          </span>
        </div>
      </div>
    </aside>
  );
}
