/**
 * O que está no editor em foco, para o chat: arquivo, linhas selecionadas,
 * texto da seleção e os erros/avisos do Monaco (markers) desse arquivo.
 * O `GroupEditor` com o foco atualiza; o composer lê para os chips
 * "arquivo em foco" e "erros do editor" e para o Alt+K "Adicionar à conversa".
 */
import { create } from "zustand";
import type { Path } from "@/api";

export interface EditorMarker {
  line: number;
  /** "erro" ou "aviso". */
  severity: "erro" | "aviso";
  message: string;
}

export interface EditorFocus {
  path: Path;
  /** Linhas da seleção (1 = primeira); iguais quando é só o cursor. */
  startLine: number;
  endLine: number;
  /** Texto selecionado (vazio = sem seleção). */
  selection: string;
  /** Linguagem do Monaco (para a cerca de código). */
  language: string;
  markers: EditorMarker[];
}

interface EditorContextState {
  focus: EditorFocus | null;
  /** Chip "arquivo em foco" ligado (anexa ao enviar). */
  attachFocus: boolean;
  /** Chip "erros do editor" ligado (anexa ao enviar). */
  attachMarkers: boolean;
  setFocus(f: EditorFocus | null): void;
  setAttachFocus(on: boolean): void;
  setAttachMarkers(on: boolean): void;
}

export const useEditorContext = create<EditorContextState>()((set) => ({
  focus: null,
  attachFocus: true,
  attachMarkers: false,
  setFocus: (focus) => set({ focus }),
  setAttachFocus: (attachFocus) => set({ attachFocus }),
  setAttachMarkers: (attachMarkers) => set({ attachMarkers }),
}));

/** Máximo de caracteres de um trecho levado ao pedido. */
const SNIPPET_MAX = 8000;
/** Máximo de markers levados ao pedido. */
const MARKERS_MAX = 30;

/** Caminho relativo à pasta (com "/"), ou o caminho inteiro se estiver fora dela. */
export function relativeTo(path: Path, root: Path | null | undefined): string {
  const norm = (p: string) => p.replace(/\\/g, "/").replace(/\/+$/, "");
  const p = norm(path);
  if (!root) return p;
  const r = norm(root);
  return p.toLowerCase().startsWith(r.toLowerCase() + "/") ? p.slice(r.length + 1) : p;
}

/** "L10" ou "L10-20". */
export function lineRange(start: number, end: number): string {
  return start === end ? `L${start}` : `L${start}-${end}`;
}

/** Referência `@caminho#L10-20` (sem linhas quando não há seleção). */
export function focusRef(f: EditorFocus, root: Path | null | undefined, withLines = f.selection !== ""): string {
  const rel = relativeTo(f.path, root);
  return withLines ? `@${rel}#${lineRange(f.startLine, f.endLine)}` : `@${rel}`;
}

/** Cerca de código com o trecho (cortado em `SNIPPET_MAX`). */
export function fenced(text: string, language: string): string {
  const body = text.length > SNIPPET_MAX ? `${text.slice(0, SNIPPET_MAX)}\n… (trecho cortado)` : text;
  const fence = body.includes("```") ? "````" : "```";
  return `${fence}${language === "plaintext" ? "" : language}\n${body.replace(/\n$/, "")}\n${fence}`;
}

/** Texto do Alt+K: a referência e, havendo seleção, o trecho. */
export function addToChatText(f: EditorFocus, root: Path | null | undefined): string {
  const ref = focusRef(f, root, true);
  return f.selection ? `${ref}\n${fenced(f.selection, f.language)}\n` : `${ref} `;
}

/** Bloco anexado ao pedido pelo chip "arquivo em foco". */
export function focusAttachment(f: EditorFocus, root: Path | null | undefined): string {
  const ref = focusRef(f, root);
  return f.selection ? `[Arquivo em foco no editor] ${ref}\n${fenced(f.selection, f.language)}` : `[Arquivo em foco no editor] ${ref}`;
}

/** Bloco anexado ao pedido pelo chip "erros do editor". Vazio sem markers. */
export function markersAttachment(f: EditorFocus, root: Path | null | undefined): string {
  if (!f.markers.length) return "";
  const rel = relativeTo(f.path, root);
  const lines = f.markers.slice(0, MARKERS_MAX).map((m) => `- ${rel}:${m.line} (${m.severity}): ${m.message.replace(/\s+/g, " ").trim()}`);
  const more = f.markers.length > MARKERS_MAX ? `\n- … e mais ${f.markers.length - MARKERS_MAX}` : "";
  return `[Problemas apontados pelo editor em ${rel}]\n${lines.join("\n")}${more}`;
}

/** Pedido com os anexos ligados (arquivo em foco e erros do editor). */
export function withEditorContext(prompt: string, root: Path | null | undefined): string {
  const s = useEditorContext.getState();
  const f = s.focus;
  if (!f) return prompt;
  const parts = [prompt];
  // O Alt+K já pôs a referência no texto: não repete.
  if (s.attachFocus && !prompt.includes(focusRef(f, root))) parts.push(focusAttachment(f, root));
  if (s.attachMarkers) {
    const m = markersAttachment(f, root);
    if (m) parts.push(m);
  }
  return parts.join("\n\n");
}
