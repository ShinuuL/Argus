/**
 * Tema de cores: o atributo `data-theme` em <html> escolhe o bloco de tokens de
 * `styles/tokens.css`. A escolha fica no localStorage.
 */
import { create } from "zustand";

export type ThemeId = "bento" | "oceano" | "ciano" | "ambar" | "violeta";

export const THEMES: { id: ThemeId; label: string; hint: string }[] = [
  { id: "bento", label: "Bento", hint: "cinza e violeta" },
  { id: "oceano", label: "Oceano", hint: "azul" },
  { id: "ciano", label: "Ardósia", hint: "ardósia e ciano" },
  { id: "ambar", label: "Grafite", hint: "grafite e âmbar" },
  { id: "violeta", label: "Noite", hint: "noite e violeta" },
];

const STORAGE_KEY = "argus.tema";
/* Quem já tem tema salvo no localStorage mantém a escolha; só quem nunca escolheu cai no Bento. */
const DEFAULT_THEME: ThemeId = "bento";

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

const ACCENT_KEY = "argus.acento";
const HEX = /^#[0-9a-f]{6}$/i;
/** Cores sobrescritas no Bento; --accent-soft e --bg-selected derivam delas. */
const ACCENT_VARS = ["--accent", "--accent-hover", "--accent-text", "--on-accent", "--selection", "--focus"] as const;

function storedAccent(): string | null {
  try {
    const v = localStorage.getItem(ACCENT_KEY);
    return v && HEX.test(v) ? v.toLowerCase() : null;
  } catch {
    return null;
  }
}

/** Texto escuro sobre acentos claros, texto claro sobre acentos escuros. */
function onAccent(hex: string): string {
  const linear = [1, 3, 5].map((i) => {
    const c = parseInt(hex.slice(i, i + 2), 16) / 255;
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  });
  const luminance = 0.2126 * linear[0] + 0.7152 * linear[1] + 0.0722 * linear[2];
  return luminance > 0.42 ? "#0d0d10" : "#ececf1";
}

/** Só o Bento aceita acento livre; nos outros temas os valores vêm do CSS. */
function applyAccent(theme: ThemeId, hex: string | null): void {
  const style = document.documentElement.style;
  if (theme !== "bento" || !hex) {
    for (const v of ACCENT_VARS) style.removeProperty(v);
    return;
  }
  style.setProperty("--accent", hex);
  style.setProperty("--accent-hover", `color-mix(in srgb, ${hex} 70%, var(--text))`);
  style.setProperty("--accent-text", `color-mix(in srgb, ${hex} 55%, var(--text))`);
  style.setProperty("--on-accent", onAccent(hex));
  style.setProperty("--selection", `color-mix(in srgb, ${hex} 25%, var(--surface-2))`);
  style.setProperty("--focus", `color-mix(in srgb, ${hex} 85%, var(--text))`);
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
  /** Acento escolhido no Bento; null é o violeta aprovado no mockup. */
  accentColor: string | null;
  setAccentColor(hex: string | null): void;
  uiScale: UiScale;
  setUiScale(v: UiScale): void;
  codeFont: CodeFont;
  setCodeFont(v: CodeFont): void;
}

export const useTheme = create<ThemeState>()((set, get) => ({
  theme: stored(),
  accentColor: storedAccent(),
  setTheme: (theme) => {
    apply(theme);
    applyAccent(theme, get().accentColor);
    try {
      localStorage.setItem(STORAGE_KEY, theme);
    } catch {
      // sem armazenamento: o tema vale só nesta abertura
    }
    set({ theme });
  },
  setAccentColor: (hex) => {
    const valid = hex && HEX.test(hex) ? hex.toLowerCase() : null;
    applyAccent(get().theme, valid);
    try {
      if (valid) localStorage.setItem(ACCENT_KEY, valid);
      else localStorage.removeItem(ACCENT_KEY);
    } catch {
      // sem armazenamento: a cor vale só nesta abertura
    }
    set({ accentColor: valid });
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
applyAccent(useTheme.getState().theme, useTheme.getState().accentColor);
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
