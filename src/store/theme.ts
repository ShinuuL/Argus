/**
 * Tema de cores: o atributo `data-theme` em <html> escolhe o bloco de tokens de
 * `styles/tokens.css`. A escolha fica no localStorage.
 */
import { create } from "zustand";

export type ThemeId = "oceano" | "ciano" | "ambar" | "violeta";

export const THEMES: { id: ThemeId; label: string; hint: string }[] = [
  { id: "oceano", label: "Oceano", hint: "azul" },
  { id: "ciano", label: "Ardósia", hint: "ardósia e ciano" },
  { id: "ambar", label: "Grafite", hint: "grafite e âmbar" },
  { id: "violeta", label: "Noite", hint: "noite e violeta" },
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

/** Tamanho da interface (zoom da página, em %). */
export const UI_SCALES = [90, 100, 110, 125] as const;
export type UiScale = (typeof UI_SCALES)[number];
/** Tamanho da fonte do editor e dos terminais (px). */
export const CODE_FONTS = [12, 13, 14, 16] as const;
export type CodeFont = (typeof CODE_FONTS)[number];

function storedNum<T extends number>(key: string, allowed: readonly T[], fallback: T): T {
  try {
    const v = Number(localStorage.getItem(key));
    return (allowed as readonly number[]).includes(v) ? (v as T) : fallback;
  } catch {
    return fallback;
  }
}

function save(key: string, v: string): void {
  try {
    localStorage.setItem(key, v);
  } catch {
    // sem armazenamento: vale só nesta abertura
  }
}

/** Zoom nativo do WebView (o `zoom` do CSS desalinha o mouse no Monaco e no xterm). */
function applyScale(scale: UiScale): void {
  void import("@tauri-apps/api/webview")
    .then((m) => m.getCurrentWebview().setZoom(scale / 100))
    .catch(() => undefined);
}

export interface ThemeState {
  theme: ThemeId;
  setTheme(theme: ThemeId): void;
  uiScale: UiScale;
  setUiScale(v: UiScale): void;
  codeFont: CodeFont;
  setCodeFont(v: CodeFont): void;
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
  uiScale: storedNum("argus.escala", UI_SCALES, 100),
  setUiScale: (uiScale) => {
    applyScale(uiScale);
    save("argus.escala", String(uiScale));
    set({ uiScale });
  },
  codeFont: storedNum("argus.fonteCodigo", CODE_FONTS, 13),
  setCodeFont: (codeFont) => {
    save("argus.fonteCodigo", String(codeFont));
    set({ codeFont });
  },
}));

// Aplica antes da primeira renderização.
apply(useTheme.getState().theme);
applyScale(useTheme.getState().uiScale);

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
