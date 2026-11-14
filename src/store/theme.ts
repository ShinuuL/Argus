/**
 * Tema de cores: o atributo `data-theme` em <html> escolhe o bloco de tokens de
 * `styles/tokens.css`. A escolha fica no localStorage.
 */
import { create } from "zustand";

export type ThemeId = "oceano" | "nebulosa" | "brasa";

export const THEMES: { id: ThemeId; label: string; hint: string }[] = [
  { id: "oceano", label: "Oceano", hint: "azul" },
  { id: "nebulosa", label: "Nebulosa", hint: "roxo" },
  { id: "brasa", label: "Brasa", hint: "preto e vermelho" },
];

const STORAGE_KEY = "argus.tema";
const DEFAULT_THEME: ThemeId = "oceano";

function isTheme(v: unknown): v is ThemeId {
  return THEMES.some((t) => t.id === v);
}

function stored(): ThemeId {
  try {
    const v = localStorage.getItem(STORAGE_KEY);
    return isTheme(v) ? v : DEFAULT_THEME;
  } catch {
    return DEFAULT_THEME;
  }
}

function apply(theme: ThemeId): void {
  document.documentElement.dataset.theme = theme;
}

export interface ThemeState {
  theme: ThemeId;
  setTheme(theme: ThemeId): void;
}

export const useTheme = create<ThemeState>()((set) => ({
  theme: stored(),
  setTheme: (theme) => {
    apply(theme);
    try {
      localStorage.setItem(STORAGE_KEY, theme);
    } catch {
      // sem armazenamento: o tema vale só nesta abertura
    }
    set({ theme });
  },
}));

// Aplica antes da primeira renderização.
apply(useTheme.getState().theme);

let probe: HTMLSpanElement | null = null;

/**
 * Valor final de um token de cor como "#rrggbb" (resolve var() e color-mix()),
 * para quem não aceita CSS: Monaco e xterm.
 */
export function tokenColor(token: string): string {
  if (!probe) {
    probe = document.createElement("span");
    probe.style.display = "none";
    document.body.appendChild(probe);
  }
  probe.style.color = `var(--${token})`;
  const css = getComputedStyle(probe).color;
  const hex = (n: number) => Math.max(0, Math.min(255, Math.round(n))).toString(16).padStart(2, "0");
  let m = /^rgba?\(\s*([\d.]+)[,\s]+([\d.]+)[,\s]+([\d.]+)/.exec(css);
  if (m) return `#${hex(+m[1])}${hex(+m[2])}${hex(+m[3])}`;
  m = /^color\(srgb\s+([\d.e-]+)\s+([\d.e-]+)\s+([\d.e-]+)/.exec(css);
  if (m) return `#${hex(+m[1] * 255)}${hex(+m[2] * 255)}${hex(+m[3] * 255)}`;
  return `#${hex(128)}${hex(128)}${hex(128)}`;
}
