import { useState, type ReactNode } from "react";
import { IconCheck, IconCopy } from "./icons";

export async function copyText(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    try {
      const ta = document.createElement("textarea");
      ta.value = text;
      ta.style.position = "fixed";
      ta.style.opacity = "0";
      document.body.appendChild(ta);
      ta.select();
      document.execCommand("copy");
      ta.remove();
      return true;
    } catch {
      return false;
    }
  }
}

export function Pane({
  label,
  actions,
  children,
  grow = 1,
}: {
  label: ReactNode;
  actions?: ReactNode;
  children: ReactNode;
  /** 同一行里多个 Pane 的宽度权重，默认 1 */
  grow?: number;
}) {
  return (
    <section style={{ flexGrow: grow }} className="flex min-w-0 flex-1 flex-col bg-panel">
      <div className="flex h-[38px] shrink-0 items-center justify-between border-b border-line pl-3.5 pr-2">
        <div className="flex min-w-0 items-center gap-2 text-[11px] font-medium tracking-wide text-muted">
          {label}
        </div>
        <div className="flex items-center gap-0.5">{actions}</div>
      </div>
      <div className="min-h-0 flex-1">{children}</div>
    </section>
  );
}

export function Segmented<T extends string>({
  value,
  onChange,
  options,
}: {
  value: T;
  onChange: (v: T) => void;
  options: { value: T; label: string }[];
}) {
  return (
    <div className="flex items-center gap-0.5 rounded-[7px] bg-[#f1efea] p-[2px]">
      {options.map((o) => (
        <button
          key={o.value}
          onClick={() => onChange(o.value)}
          className={`whitespace-nowrap rounded-[5px] px-2.5 py-[3px] text-[12px] transition-colors ${
            value === o.value
              ? "bg-panel text-ink shadow-[0_1px_2px_rgba(23,23,20,0.08)]"
              : "text-muted hover:text-ink"
          }`}
        >
          {o.label}
        </button>
      ))}
    </div>
  );
}

export function Select<T extends string>({
  value,
  onChange,
  options,
  disabled,
  title,
}: {
  value: T;
  onChange: (v: T) => void;
  options: { value: T; label: string }[];
  disabled?: boolean;
  title?: string;
}) {
  return (
    <div
      title={title}
      className={`relative flex items-center rounded-[6px] border border-line bg-panel transition-colors ${
        disabled ? "opacity-45" : "focus-within:border-line-strong hover:border-line-strong"
      }`}
    >
      <select
        value={value}
        disabled={disabled}
        onChange={(e) => onChange(e.target.value as T)}
        className="cursor-pointer appearance-none whitespace-nowrap bg-transparent py-[3px] pl-2 pr-6 text-[11.5px] text-ink outline-none disabled:cursor-default"
      >
        {options.map((o) => (
          <option key={o.value} value={o.value}>
            {o.label}
          </option>
        ))}
      </select>
      <svg
        viewBox="0 0 16 16"
        className="pointer-events-none absolute right-[6px] h-[10px] w-[10px] text-faint"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.8"
        strokeLinecap="round"
        strokeLinejoin="round"
      >
        <path d="M4 6.4 8 10.2 12 6.4" />
      </svg>
    </div>
  );
}

export function Chip({
  active,
  onClick,
  children,
}: {
  active?: boolean;
  onClick?: () => void;
  children: ReactNode;
}) {
  return (
    <button
      onClick={onClick}
      className={`rounded-[6px] border px-2 py-[3px] font-mono text-[11.5px] uppercase transition-colors ${
        active
          ? "border-accent/40 bg-accent-soft text-accent"
          : "border-line text-muted hover:border-line-strong hover:text-ink"
      }`}
    >
      {children}
    </button>
  );
}

export function Toggle({
  checked,
  onChange,
  children,
}: {
  checked: boolean;
  onChange: (v: boolean) => void;
  children: ReactNode;
}) {
  return (
    <button
      onClick={() => onChange(!checked)}
      className="flex items-center gap-1.5 rounded-[6px] px-1.5 py-1 text-[12px] text-muted transition-colors hover:text-ink"
    >
      <span
        className={`flex h-[13px] w-[13px] items-center justify-center rounded-[4px] border transition-colors ${
          checked ? "border-accent bg-accent text-white" : "border-line-strong bg-panel"
        }`}
      >
        {checked ? (
          <svg viewBox="0 0 16 16" className="h-[9px] w-[9px]" fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round" strokeLinejoin="round">
            <path d="M3.4 8.4 6.6 11.4 12.6 4.8" />
          </svg>
        ) : null}
      </span>
      {children}
    </button>
  );
}

export function IconBtn({
  onClick,
  title,
  children,
  active,
}: {
  onClick?: () => void;
  title?: string;
  children: ReactNode;
  active?: boolean;
}) {
  return (
    <button
      onClick={onClick}
      title={title}
      className={`flex h-[26px] w-[26px] items-center justify-center rounded-[6px] transition-colors ${
        active ? "bg-accent-soft text-accent" : "text-muted hover:bg-[#f2f0eb] hover:text-ink"
      }`}
    >
      {children}
    </button>
  );
}

export function TextBtn({
  onClick,
  children,
  title,
}: {
  onClick?: () => void;
  children: ReactNode;
  title?: string;
}) {
  return (
    <button
      onClick={onClick}
      title={title}
      className="rounded-[6px] px-2 py-1 text-[12px] text-muted transition-colors hover:bg-[#f2f0eb] hover:text-ink"
    >
      {children}
    </button>
  );
}

export function PrimaryBtn({
  onClick,
  disabled,
  title,
  children,
}: {
  onClick?: () => void;
  disabled?: boolean;
  title?: string;
  children: ReactNode;
}) {
  return (
    <button
      onClick={onClick}
      disabled={disabled}
      title={title}
      className="whitespace-nowrap rounded-[7px] bg-accent px-3 py-[5px] text-[12px] font-medium text-white transition-opacity hover:opacity-90 disabled:opacity-50"
    >
      {children}
    </button>
  );
}

export function CopyBtn({ text, label = "复制" }: { text: string; label?: string }) {
  const [done, setDone] = useState(false);
  return (
    <button
      onClick={async () => {
        const ok = await copyText(text);
        if (ok) {
          setDone(true);
          setTimeout(() => setDone(false), 1200);
        }
      }}
      className={`flex items-center gap-1 rounded-[6px] px-2 py-1 text-[12px] transition-colors ${
        done ? "text-accent" : "text-muted hover:bg-[#f2f0eb] hover:text-ink"
      }`}
    >
      {done ? <IconCheck className="h-[13px] w-[13px]" /> : <IconCopy className="h-[13px] w-[13px]" />}
      {done ? "已复制" : label}
    </button>
  );
}

export function Kbd({ children }: { children: ReactNode }) {
  return (
    <kbd className="rounded-[4px] border border-line bg-[#f6f4f0] px-1 py-[1px] font-sans text-[10px] leading-[14px] text-muted">
      {children}
    </kbd>
  );
}

export function Hint({ tone = "muted", children }: { tone?: "muted" | "warn" | "danger"; children: ReactNode }) {
  const tones = {
    muted: "text-muted",
    warn: "text-warn",
    danger: "text-danger",
  } as const;
  return <span className={`text-[11.5px] ${tones[tone]}`}>{children}</span>;
}
