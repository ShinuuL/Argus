/** Ícones em SVG inline (traço, viewBox 24). Sem biblioteca: só os que a interface usa. */

const PATHS = {
  sidebarLeft: "M4 5h16v14H4zM9 5v14",
  sidebarRight: "M4 5h16v14H4zM15 5v14",
  terminal: "M4 5h16v14H4zM7 10l3 2-3 2M12 15h4",
  plus: "M12 5v14M5 12h14",
  search: "M11 4a7 7 0 110 14 7 7 0 010-14zM20 20l-4-4",
  folder: "M3 6h7l2 2h9v11H3z",
  folderOpen: "M3 19V6h7l2 2h8v3M3 19l3-8h16l-3 8z",
  collapse: "M7 9l5-5 5 5M7 15l5 5 5-5",
  filter: "M4 6h16M7 12h10M10 18h4",
  chevronRight: "M9 6l6 6-6 6",
  chevronDown: "M6 9l6 6 6-6",
  chat: "M4 5h16v11H9l-5 4z",
  grid: "M4 4h7v7H4zM13 4h7v7h-7zM4 13h7v7H4zM13 13h7v7h-7z",
  user: "M12 3a4 4 0 110 8 4 4 0 010-8zM4 21c0-4 4-7 8-7s8 3 8 7",
  box: "M12 3l8 4.5v9L12 21l-8-4.5v-9zM12 12l8-4.5M12 12v9M12 12L4 7.5",
  chart: "M4 20V10M10 20V4M16 20v-7M22 20H2",
  phone: "M7 2h10v20H7zM11 18h2",
  gear: "M12 9a3 3 0 110 6 3 3 0 010-6zM12 2v3M12 19v3M2 12h3M19 12h3M4.9 4.9l2.1 2.1M17 17l2.1 2.1M4.9 19.1L7 17M17 7l2.1-2.1",
  clip: "M20 11l-8.5 8.5a5 5 0 01-7-7L13 4a3.3 3.3 0 014.7 4.7L9.5 17a1.7 1.7 0 01-2.4-2.4L14.5 7",
  send: "M12 19V5M6 11l6-6 6 6",
  stop: "M7 7h10v10H7z",
  file: "M6 3h8l4 4v14H6zM14 3v4h4",
  diff: "M8 4v10M3 9h10M15 18h6M5 20h6",
  tree: "M6 3v12a3 3 0 003 3h3M6 9h6M12 6h8v6h-8zM12 15h8v6h-8z",
  server: "M4 4h16v6H4zM4 14h16v6H4zM7 7h1M7 17h1",
  wrench: "M14 6a4 4 0 005 5l-9 9a2.1 2.1 0 01-3-3l9-9a4 4 0 01-2-2z",
  pencil: "M4 20l1-5L16 4l4 4L9 19zM14 6l4 4",
  eye: "M2 12s4-7 10-7 10 7 10 7-4 7-10 7S2 12 2 12zM12 9a3 3 0 110 6 3 3 0 010-6z",
  globe: "M12 3a9 9 0 110 18 9 9 0 010-18zM3 12h18M12 3c3 3 3 15 0 18M12 3c-3 3-3 15 0 18",
  bot: "M6 8h12v11H6zM12 8V4M9 13h1M14 13h1M3 13v3M21 13v3",
  alert: "M12 3l10 18H2zM12 10v5M12 18v.5",
  check: "M4 12l5 5L20 6",
  close: "M6 6l12 12M18 6L6 18",
  splitH: "M4 5h16v14H4zM12 5v14",
  splitV: "M4 5h16v14H4zM4 12h16",
  refresh: "M20 11a8 8 0 10-2.3 5.7M20 4v7h-7",
  shield: "M12 3l8 3v6c0 5-3.5 8-8 9-4.5-1-8-4-8-9V6z",
  loop: "M17 2l4 4-4 4M3 11V9a3 3 0 013-3h15M7 22l-4-4 4-4M21 13v2a3 3 0 01-3 3H3",
  palette: "M12 3a9 9 0 100 18c1.5 0 2-1 2-2s-.5-1.5-.5-2.5S14.5 15 16 15h2a3 3 0 003-3c0-5-4-9-9-9zM7.5 11v.5M10 7.5v.5M14.5 7.5v.5",
} as const;

export type IconName = keyof typeof PATHS;

export interface IconProps {
  name: IconName;
  size?: number;
  /** Preenche em vez de traçar (ex.: botão Parar). */
  filled?: boolean;
  className?: string;
}

export function Icon({ name, size = 16, filled = false, className }: IconProps) {
  return (
    <svg
      className={className ? `ic ${className}` : "ic"}
      viewBox="0 0 24 24"
      width={size}
      height={size}
      fill={filled ? "currentColor" : "none"}
      stroke="currentColor"
      strokeWidth="1.7"
      strokeLinejoin="round"
      strokeLinecap="round"
      aria-hidden="true"
    >
      <path d={PATHS[name]} />
    </svg>
  );
}

/** Ícone de uma ferramenta do agente, pelo nome (Read, Edit, Bash, Grep, WebFetch, Agent...). */
export function toolIcon(tool: string): IconName {
  const t = tool.toLowerCase();
  if (/bash|shell|exec|command|terminal/.test(t)) return "terminal";
  if (/edit|write|patch|apply|notebook/.test(t)) return "pencil";
  if (/read|view|cat|open/.test(t)) return "eye";
  if (/grep|glob|search|find|ls|list/.test(t)) return "search";
  if (/web|fetch|url|http/.test(t)) return "globe";
  if (/agent|task/.test(t)) return "bot";
  return "wrench";
}
