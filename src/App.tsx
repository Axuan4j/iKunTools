import { useEffect, useState } from "react";
import Sidebar from "./shell/Sidebar";
import CommandPalette from "./components/CommandPalette";
import { IconSearch } from "./components/icons";
import { Kbd } from "./components/ui";
import { TOOLS } from "./features/registry";

export default function App() {
  const [activeId, setActiveId] = useState(() => {
    const fromHash = window.location.hash.replace("#", "").split("/")[0];
    return TOOLS.some((t) => t.id === fromHash) ? fromHash : TOOLS[0].id;
  });
  const [paletteOpen, setPaletteOpen] = useState(false);

  const tool = TOOLS.find((t) => t.id === activeId) ?? TOOLS[0];
  const Active = tool.Component;

  useEffect(() => {
    if (window.location.hash !== `#${activeId}`) {
      window.history.replaceState(null, "", `#${activeId}`);
    }
  }, [activeId]);

  useEffect(() => {
    const onHash = () => {
      const id = window.location.hash.replace("#", "").split("/")[0];
      if (TOOLS.some((t) => t.id === id)) setActiveId(id);
    };
    window.addEventListener("hashchange", onHash);
    return () => window.removeEventListener("hashchange", onHash);
  }, []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setPaletteOpen((v) => !v);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  return (
    <div className="flex h-full w-full overflow-hidden bg-page">
      <Sidebar activeId={activeId} onSelect={setActiveId} onOpenPalette={() => setPaletteOpen(true)} />

      <main className="flex min-w-0 flex-1 flex-col">
        <header
          data-tauri-drag-region
          className="flex h-[56px] shrink-0 items-center justify-between border-b border-line bg-page px-5"
        >
          <div data-tauri-drag-region className="flex min-w-0 items-center gap-2">
            <h1 className="truncate text-[13px] font-medium tracking-[-0.01em] text-ink">{tool.name}</h1>
            <span className="shrink-0 rounded-[4px] bg-[#efedE8] px-[5px] py-[1px] text-[10px] text-muted">
              {tool.group}
            </span>
          </div>

          <div className="flex shrink-0 items-center gap-3 pl-4">
            <button
              onClick={() => setPaletteOpen(true)}
              className="flex items-center gap-1.5 rounded-[7px] border border-line bg-panel px-2 py-[4px] text-[11.5px] text-muted transition-colors hover:border-line-strong"
            >
              <IconSearch className="h-[13px] w-[13px]" />
              切换
              <Kbd>⌘K</Kbd>
            </button>
          </div>
        </header>

        <div className="min-h-0 flex-1">
          <Active />
        </div>
      </main>

      <CommandPalette
        open={paletteOpen}
        tools={TOOLS}
        onClose={() => setPaletteOpen(false)}
        onPick={setActiveId}
      />
    </div>
  );
}
