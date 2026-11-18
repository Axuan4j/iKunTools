import type { ReactNode } from "react";

type P = { className?: string };

const base = "h-4 w-4";

const svg = (children: ReactNode) => (p: P) => (
  <svg
    viewBox="0 0 16 16"
    fill="none"
    stroke="currentColor"
    strokeWidth="1.25"
    strokeLinecap="round"
    strokeLinejoin="round"
    className={`${base} ${p.className ?? ""}`}
  >
    {children}
  </svg>
);

export const IconBraces = svg(
  <>
    <path d="M6 2.5c-1.2 0-1.6.6-1.6 1.7v1.4c0 1-.4 1.4-1.2 1.4.8 0 1.2.4 1.2 1.4v1.4c0 1.1.4 1.7 1.6 1.7" />
    <path d="M10 2.5c1.2 0 1.6.6 1.6 1.7v1.4c0 1 .4 1.4 1.2 1.4-.8 0-1.2.4-1.2 1.4v1.4c0 1.1-.4 1.7-1.6 1.7" />
  </>,
);

export const IconHash = svg(
  <>
    <path d="M5.8 2.5 4.6 13.5M11 2.5 9.8 13.5M2.5 6h11M2 10h11" />
  </>,
);

export const IconLock = svg(
  <>
    <rect x="3.2" y="7" width="9.6" height="6.6" rx="1.6" />
    <path d="M5.6 7V5.4a2.4 2.4 0 0 1 4.8 0V7" />
  </>,
);

export const IconBinary = svg(
  <>
    <rect x="2.5" y="2.5" width="4.4" height="5" rx="1" />
    <rect x="9.1" y="8.5" width="4.4" height="5" rx="1" />
    <path d="M9.1 2.5h4.4v5H9.1zM2.5 8.5h4.4v5H2.5z" opacity="0" />
    <path d="M11.3 2.5v5M4.7 8.5v5" />
  </>,
);

export const IconKey = svg(
  <>
    <circle cx="5.4" cy="10.6" r="2.6" />
    <path d="M7.3 8.8 12.8 3.3M10.4 5.6l1.5 1.5M11.9 4.2l1.3 1.3" />
  </>,
);

export const IconShield = svg(
  <>
    <path d="M8 2.2 13.1 4v4.2c0 2.9-2.1 5.1-5.1 5.8-3-.7-5.1-2.9-5.1-5.8V4z" />
    <path d="M6.1 8.1 7.5 9.5l2.5-2.6" />
  </>,
);

export const IconLink = svg(
  <>
    <path d="M6.7 9.3a2.5 2.5 0 0 0 3.5 0l1.9-1.9a2.5 2.5 0 0 0-3.5-3.5l-1 1" />
    <path d="M9.3 6.7a2.5 2.5 0 0 0-3.5 0L3.9 8.6a2.5 2.5 0 0 0 3.5 3.5l1-1" />
  </>,
);

export const IconCode = svg(
  <>
    <path d="M5.7 4.4 2.6 8l3.1 3.6M10.3 4.4 13.4 8l-3.1 3.6" />
  </>,
);

export const IconRegex = svg(
  <>
    <circle cx="4.1" cy="11.5" r="1.15" />
    <path d="M9.7 4.9v5.6M7.2 6.3l5 2.8M12.2 6.3l-5 2.8" />
  </>,
);

export const IconXml = svg(
  <>
    <path d="M4.9 4.3 2 8l2.9 3.7" />
    <path d="M11.1 4.3 14 8l-2.9 3.7" />
    <path d="M9.3 3.4 6.7 12.6" />
  </>,
);

export const IconClock = svg(
  <>
    <circle cx="8" cy="8" r="5.6" />
    <path d="M8 4.9V8l2.2 1.5" />
  </>,
);

export const IconSearch = svg(
  <>
    <circle cx="7.2" cy="7.2" r="4.2" />
    <path d="M10.4 10.4 13.4 13.4" />
  </>,
);

export const IconDice = svg(
  <>
    <rect x="2.6" y="2.6" width="10.8" height="10.8" rx="2.4" />
    <circle cx="6" cy="6" r="0.85" fill="currentColor" stroke="none" />
    <circle cx="10" cy="10" r="0.85" fill="currentColor" stroke="none" />
    <circle cx="10" cy="6" r="0.85" fill="currentColor" stroke="none" />
    <circle cx="6" cy="10" r="0.85" fill="currentColor" stroke="none" />
  </>,
);

export const IconGlobe = svg(
  <>
    <circle cx="8" cy="8" r="5.6" />
    <path d="M2.6 8h10.8" />
    <path d="M8 2.4c1.7 1.6 2.5 3.5 2.5 5.6S9.7 12 8 13.6C6.3 12 5.5 10.1 5.5 8S6.3 4 8 2.4Z" />
  </>,
);

export const IconPin = svg(
  <>
    <path d="M8 13.6s4.2-4 4.2-7.1A4.2 4.2 0 0 0 3.8 6.5c0 3.1 4.2 7.1 4.2 7.1Z" />
    <circle cx="8" cy="6.4" r="1.5" />
  </>,
);

export const IconChevron = svg(<path d="M6 3.5 10.5 8 6 12.5" />);

export const IconCopy = svg(
  <>
    <rect x="5.6" y="5.6" width="7.6" height="7.6" rx="1.6" />
    <path d="M10.4 3.2H4.2a1.2 1.2 0 0 0-1.2 1.2v6.2" />
  </>,
);

export const IconCheck = svg(<path d="M3.4 8.4 6.6 11.4 12.6 4.8" />);

export const IconClose = svg(<path d="M4 4l8 8M12 4l-8 8" />);

export const IconBroom = svg(
  <>
    <path d="M3 13.2h10M4.4 13.2c0-2.4 1.4-3.6 1.4-5.6M8 13.2c0-2.6.6-4.2 1-5.6" />
    <path d="M10.6 7.4 12.8 3" />
  </>,
);

export const IconWand = svg(
  <>
    <path d="M3.2 12.8 9.4 6.6M10.6 5.4l.9-.9" />
    <path d="M11.8 2.2v1.8M13.8 5.4h-1.8M12.4 8.6l-.8-.8" />
  </>,
);

export const IconSend = svg(
  <>
    <path d="M13.4 2.6 6.8 9.2" />
    <path d="M13.4 2.6 9.6 13.4 7.6 9.6 3.4 7.6 13.4 2.6Z" />
  </>,
);

export const IconTerminal = svg(
  <>
    <rect x="2.2" y="3.2" width="11.6" height="9.6" rx="1.6" />
    <path d="M5 6.6 6.8 8.4 5 10.2M8.6 10.4h2.6" />
  </>,
);

export const IconHistory = svg(
  <>
    <path d="M2.6 8a5.4 5.4 0 1 0 1.7-3.9" />
    <path d="M2.4 2.6v2.8h2.8" />
    <path d="M8 5.4V8l1.9 1.2" />
  </>,
);
