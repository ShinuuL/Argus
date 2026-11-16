/**
 * Anexos do chat: arquivos (clipe, arrastar e soltar, colar), prints colados e
 * o texto colado grande, que vira um .txt em vez de lotar a caixa.
 *
 * Nesta leva os anexos vão ao agente PELO CAMINHO: o arquivo é gravado na pasta
 * de anexos do Argus e o pedido ganha, no fim, o bloco "Arquivos anexados pelo
 * caminho". `splitPrompt` desfaz isso para a conversa mostrar chips e miniaturas.
 */
import { baseName, errorMessage, type Path, type TaskId } from "@/api";
import { chatAttachmentRead, chatAttachmentSave } from "./backend";

export type AttachmentKind = "image" | "pdf" | "text" | "file";

/** Anexo na caixa de composição (antes de enviar). */
export interface ChatAttachment {
  id: string;
  kind: AttachmentKind;
  name: string;
  /** Bytes. */
  size: number;
  /** Caminho gravado; null enquanto grava ou se a gravação falhou. */
  path: Path | null;
  status: "saving" | "ready" | "error";
  error?: string;
  /** Conteúdo, só para texto (permite "colar como texto" e o envio embutido). */
  text?: string;
  /** Veio de um texto colado (mostra o botão de voltar a ser texto). */
  pasted?: boolean;
  /** URL da miniatura (imagens). */
  previewUrl?: string;
}

/** Anexo já enviado, como aparece na mensagem. */
export interface SentAttachment {
  kind: AttachmentKind;
  name: string;
  path: Path;
}

/** Limite por anexo (o mesmo do backend). */
export const MAX_BYTES = 8 * 1024 * 1024;
/** Texto colado com mais que isso de linhas (ou muito comprido) vira anexo. */
export const PASTE_LINES = 2;
export const PASTE_CHARS = 2000;

const IMAGE_OK = ["image/png", "image/jpeg", "image/gif", "image/webp"];
const IMAGE_EXT = new Set(["png", "jpg", "jpeg", "gif", "webp", "bmp"]);
const TEXT_EXT = new Set(
  "txt md markdown csv tsv json jsonl jsonc xml html htm css scss less js mjs cjs ts tsx jsx vue svelte py rb php java kt kts cs go rs c h cpp hpp cc swift dart lua sql sh bash zsh ps1 psm1 bat cmd yml yaml toml ini cfg conf log env properties gradle lock graphql proto dockerfile makefile gitignore editorconfig".split(
    " ",
  ),
);

export function isBigPaste(text: string): boolean {
  return text.split(/\r?\n/).length > PASTE_LINES || text.length > PASTE_CHARS;
}

export function sizeText(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1).replace(".", ",")} MB`;
}

function ext(name: string): string {
  const dot = name.lastIndexOf(".");
  return dot >= 0 ? name.slice(dot + 1).toLowerCase() : name.toLowerCase();
}

/** Tipo do anexo pelo nome do arquivo. */
export function kindOfName(name: string): AttachmentKind {
  const e = ext(name);
  if (IMAGE_EXT.has(e)) return "image";
  if (e === "pdf") return "pdf";
  if (TEXT_EXT.has(e)) return "text";
  return "file";
}

const KIND_LABEL: Record<AttachmentKind, string> = { image: "imagem", pdf: "PDF", text: "texto", file: "arquivo" };

/** Rótulo curto do tipo (chips e bloco do pedido). */
export function kindLabel(kind: AttachmentKind): string {
  return KIND_LABEL[kind];
}

function toBase64(bytes: Uint8Array): string {
  let bin = "";
  for (let i = 0; i < bytes.length; i += 0x8000) bin += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  return btoa(bin);
}

/** Imagem em formato pouco comum (tiff, heic...): vira PNG pelo canvas. */
async function toPng(file: File): Promise<Blob> {
  const bmp = await createImageBitmap(file);
  const c = document.createElement("canvas");
  c.width = bmp.width;
  c.height = bmp.height;
  c.getContext("2d")!.drawImage(bmp, 0, 0);
  bmp.close();
  const blob = await new Promise<Blob | null>((resolve) => c.toBlob(resolve, "image/png"));
  c.width = c.height = 0;
  if (!blob) throw new Error(`${file.name}: não foi possível converter a imagem`);
  return blob;
}

let seq = 0;
const newId = () => `a${Date.now().toString(36)}${(seq++).toString(36)}`;

let prints = 0;
let pastes = 0;

/** Miniaturas já conhecidas, por caminho gravado (evita reler do disco). */
const previews = new Map<Path, string>();

/** Miniatura de uma imagem anexada: da memória, ou lida da pasta de anexos. */
export async function previewFor(path: Path): Promise<string | null> {
  const known = previews.get(path);
  if (known) return known;
  const url = await chatAttachmentRead(path);
  if (url) previews.set(path, url);
  return url;
}

export interface Prepared {
  /** Anexo como entra na caixa, ainda "saving". */
  attachment: ChatAttachment;
  /** Grava no disco e devolve o anexo final (ready ou error). */
  save: () => Promise<ChatAttachment>;
}

function prepared(
  att: ChatAttachment,
  task: TaskId | null,
  draft: string | null,
  bytes: () => Promise<Uint8Array>,
): Prepared {
  return {
    attachment: att,
    save: async () => {
      try {
        const saved = await chatAttachmentSave(task, draft, att.name, toBase64(await bytes()));
        if (att.previewUrl) previews.set(saved.path, att.previewUrl);
        return { ...att, path: saved.path, size: saved.size, status: "ready" };
      } catch (e) {
        // texto ainda pode ir embutido no pedido; o resto fica com o erro no chip
        if (att.text !== undefined) return { ...att, status: "ready", error: undefined };
        return { ...att, status: "error", error: errorMessage(e) };
      }
    },
  };
}

/**
 * Um arquivo (do clipe, solto ou colado) vira anexo. Rejeita com o motivo
 * quando o tamanho passa do limite.
 */
export async function fromFile(file: File, task: TaskId | null, draft: string | null = null): Promise<Prepared> {
  const type = file.type.toLowerCase();
  let name = file.name || "arquivo";
  let blob: Blob = file;
  // print colado chega como "image.png": ganha um nome próprio
  if (type.startsWith("image/") && /^image\.\w+$/i.test(name)) name = `print-${++prints}.${ext(name)}`;
  if (type.startsWith("image/") && !IMAGE_OK.includes(type) && ext(name) !== "bmp") {
    blob = await toPng(file);
    name = name.replace(/\.\w+$/, "") + ".png";
  }
  if (blob.size > MAX_BYTES) throw new Error(`${name}: acima de ${MAX_BYTES / 1024 / 1024} MB`);
  const kind: AttachmentKind = type.startsWith("image/") ? "image" : type === "application/pdf" ? "pdf" : kindOfName(name);
  const att: ChatAttachment = { id: newId(), kind, name, size: blob.size, path: null, status: "saving" };
  if (kind === "image") att.previewUrl = URL.createObjectURL(blob);
  if (kind === "text" && blob.size <= 256 * 1024) att.text = await blob.text();
  return prepared(att, task, draft, async () => new Uint8Array(await blob.arrayBuffer()));
}

/** Texto colado grande vira `colado-N.txt`. */
export function fromPaste(text: string, task: TaskId | null, draft: string | null = null): Prepared {
  const bytes = new TextEncoder().encode(text);
  const att: ChatAttachment = {
    id: newId(),
    kind: "text",
    name: `colado-${++pastes}.txt`,
    size: bytes.length,
    path: null,
    status: "saving",
    text,
    pasted: true,
  };
  return prepared(att, task, draft, async () => bytes);
}

/** Libera a miniatura de um anexo tirado da caixa sem ter sido enviado. */
export function discard(att: ChatAttachment): void {
  if (att.previewUrl && !att.path) URL.revokeObjectURL(att.previewUrl);
}

const PATHS_HEADER = "Arquivos anexados pelo caminho (abra com as ferramentas se precisar):";

/**
 * Monta o pedido que vai ao agente: o texto, os textos que não puderam ser
 * gravados (embutidos) e, no fim, o bloco com os caminhos dos anexos.
 */
export function buildPrompt(text: string, attachments: ChatAttachment[]): string {
  const ready = attachments.filter((a) => a.status === "ready");
  const inline = ready.filter((a) => !a.path && a.text !== undefined);
  const byPath = ready.filter((a) => a.path);
  let out = text.trim();
  for (const a of inline) out += `\n\nConteúdo de ${a.name}:\n\`\`\`\n${a.text}\n\`\`\``;
  if (byPath.length) {
    out += `\n\n${PATHS_HEADER}\n` + byPath.map((a) => `- ${a.path} (${kindLabel(a.kind)})`).join("\n");
  }
  return out.trim();
}

/** Separa o texto do usuário do bloco de anexos acrescentado por `buildPrompt`. */
export function splitPrompt(prompt: string): { text: string; attachments: SentAttachment[] } {
  const at = prompt.lastIndexOf(PATHS_HEADER);
  if (at < 0) return { text: prompt, attachments: [] };
  const attachments: SentAttachment[] = [];
  for (const line of prompt.slice(at + PATHS_HEADER.length).split("\n")) {
    const m = /^- (.+?)(?: \(([^()]*)\))?$/.exec(line.trim());
    if (!m) {
      if (line.trim()) return { text: prompt, attachments: [] }; // não é o nosso bloco
      continue;
    }
    attachments.push({ kind: kindOfName(m[1]), name: baseName(m[1]), path: m[1] });
  }
  return { text: prompt.slice(0, at).trimEnd(), attachments };
}
