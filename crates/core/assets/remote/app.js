"use strict";
/*
 * Argus pelo celular. HTML, CSS e JavaScript puros, sem biblioteca e sem nada de fora.
 * Fala com o servidor da IDE (crates/core/src/remote.rs): REST em /api e eventos
 * ao vivo pelo WebSocket /api/stream. O acesso vale pelo cookie criado em /pair.
 */

/* ============================================================ tema */
/* Roda antes da primeira pintura (o script é carregado no <head>, sem defer). */

const THEMES = [
  { id: "oceano", label: "Oceano", hint: "azul", bg: "#0d1b2a" },
  { id: "nebulosa", label: "Nebulosa", hint: "roxo", bg: "#10002b" },
  { id: "brasa", label: "Brasa", hint: "preto e vermelho", bg: "#0b090a" },
];
const THEME_KEY = "argus.tema";

/** localStorage pode não existir (modo privado, bloqueado): nunca derruba a tela. */
function loadKey(key, fallback) {
  try {
    const v = localStorage.getItem(key);
    return v == null ? fallback : v;
  } catch (_) {
    return fallback;
  }
}
function saveKey(key, value) {
  try {
    localStorage.setItem(key, value);
  } catch (_) {
    /* sem armazenamento: vale só nesta abertura */
  }
}
function loadJson(key, fallback) {
  try {
    const v = JSON.parse(loadKey(key, "null"));
    return v == null ? fallback : v;
  } catch (_) {
    return fallback;
  }
}

function currentTheme() {
  const v = loadKey(THEME_KEY, "oceano");
  return THEMES.some((t) => t.id === v) ? v : "oceano";
}

/** Aplica o tema na página, na barra do navegador (theme-color), no manifest e no ícone. */
function applyTheme(id) {
  const theme = THEMES.find((t) => t.id === id) || THEMES[0];
  document.documentElement.dataset.theme = theme.id;
  const set = (elId, attr, value) => {
    const el = document.getElementById(elId);
    if (el) el.setAttribute(attr, value);
  };
  set("theme-color", "content", theme.bg);
  set("manifest", "href", "/manifest.webmanifest?tema=" + theme.id);
  set("icon", "href", "/icon.svg?tema=" + theme.id);
  set("touch-icon", "href", "/icon.svg?tema=" + theme.id);
  return theme.id;
}
applyTheme(currentTheme());

/* ============================================================ utilidades */

const $ = (id) => document.getElementById(id);

/** Cria um elemento. `html` só é usado com o resultado de `md()` (já escapado). */
function h(tag, props, ...kids) {
  const el = document.createElement(tag);
  if (props) {
    for (const k of Object.keys(props)) {
      const v = props[k];
      if (v == null || v === false) continue;
      if (k === "class") el.className = v;
      else if (k === "text") el.textContent = v;
      else if (k === "html") el.innerHTML = v;
      else if (k.startsWith("on")) el.addEventListener(k.slice(2), v);
      else if (v === true) el.setAttribute(k, "");
      else el.setAttribute(k, v);
    }
  }
  append(el, kids);
  return el;
}
function append(el, kids) {
  for (const kid of kids) {
    if (kid == null || kid === false) continue;
    if (Array.isArray(kid)) append(el, kid);
    else el.append(kid.nodeType ? kid : document.createTextNode(String(kid)));
  }
}

/* Ícones em SVG inline (traço, viewBox 24): os mesmos desenhos do desktop. */
const ICONS = {
  back: "M15 5l-7 7 7 7",
  plus: "M12 5v14M5 12h14",
  search: "M11 4a7 7 0 110 14 7 7 0 010-14zM20 20l-4-4",
  folder: "M3 6h7l2 2h9v11H3z",
  chevronRight: "M9 6l6 6-6 6",
  chat: "M4 5h16v11H9l-5 4z",
  gear: "M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2zM12 9a3 3 0 1 1 0 6 3 3 0 0 1 0-6z",
  send: "M12 19V5M6 11l6-6 6 6",
  stop: "M7 7h10v10H7z",
  server: "M4 4h16v6H4zM4 14h16v6H4zM7 7h1M7 17h1",
  terminal: "M4 5h16v14H4zM7 10l3 2-3 2M12 15h4",
  wrench: "M14 6a4 4 0 005 5l-9 9a2.100 2.100 0 01-3-3l9-9a4 4 0 01-2-2z",
  pencil: "M4 20l1-5L16 4l4 4L9 19zM14 6l4 4",
  eye: "M2 12s4-7 10-7 10 7 10 7-4 7-10 7S2 12 2 12zM12 9a3 3 0 110 6 3 3 0 010-6z",
  globe: "M12 3a9 9 0 110 18 9 9 0 010-18zM3 12h18M12 3c3 3 3 15 0 18M12 3c-3 3-3 15 0 18",
  bot: "M6 8h12v11H6zM12 8V4M9 13h1M14 13h1M3 13v3M21 13v3",
  alert: "M12 3l10 18H2zM12 10v5M12 18v.500",
  check: "M4 12l5 5L20 6",
  close: "M6 6l12 12M18 6L6 18",
  refresh: "M20 11a8 8 0 10-2.300 5.700M20 4v7h-7",
  shield: "M12 3l8 3v6c0 5-3.500 8-8 9-4.500-1-8-4-8-9V6z",
  loop: "M17 2l4 4-4 4M3 11V9a3 3 0 013-3h15M7 22l-4-4 4-4M21 13v2a3 3 0 01-3 3H3",
  link: "M14 5h5v5M19 5l-8 8M11 7H5v12h12v-6",
  down: "M12 5v14M6 13l6 6 6-6",
  lock: "M6 11h12v9H6zM9 11V8a3 3 0 016 0v3",
  wifiOff: "M3 3l18 18M5 12a10 10 0 014-2.500M19 12a10 10 0 00-5-3M8.500 15.500a5 5 0 017 0M12 19v.500",
  file: "M6 3h8l4 4v14H6zM14 3v4h4",
  image: "M4 5h16v14H4zM4 16l4.500-4.500 4 4L16 12l4 4M15 9h.500",
};
const SVG_NS = "http://www.w3.org/2000/svg";
function icon(name, size, cls, filled) {
  const svg = document.createElementNS(SVG_NS, "svg");
  svg.setAttribute("viewBox", "0 0 24 24");
  svg.setAttribute("width", size || 20);
  svg.setAttribute("height", size || 20);
  svg.setAttribute("fill", filled ? "currentColor" : "none");
  svg.setAttribute("stroke", "currentColor");
  svg.setAttribute("stroke-width", "1.800");
  svg.setAttribute("stroke-linejoin", "round");
  svg.setAttribute("stroke-linecap", "round");
  svg.setAttribute("aria-hidden", "true");
  svg.setAttribute("class", cls ? "ic " + cls : "ic");
  const path = document.createElementNS(SVG_NS, "path");
  path.setAttribute("d", ICONS[name] || ICONS.file);
  svg.append(path);
  return svg;
}
function toolIcon(tool) {
  const t = String(tool).toLowerCase();
  if (/bash|shell|exec|command|terminal/.test(t)) return "terminal";
  if (/edit|write|patch|apply|notebook/.test(t)) return "pencil";
  if (/read|view|cat|open/.test(t)) return "eye";
  if (/grep|glob|search|find|ls|list/.test(t)) return "search";
  if (/web|fetch|url|http/.test(t)) return "globe";
  if (/agent|task/.test(t)) return "bot";
  return "wrench";
}

/** Enum do serde com tag externa: "Running" ou { Failed: "motivo" }. */
function variant(value) {
  if (typeof value === "string") return { kind: value, data: undefined };
  if (value && typeof value === "object") {
    const keys = Object.keys(value);
    if (keys.length === 1) return { kind: keys[0], data: value[keys[0]] };
  }
  return { kind: "", data: value };
}

const baseName = (p) => {
  const parts = String(p).split(/[\\/]+/).filter(Boolean);
  return parts.length ? parts[parts.length - 1] : String(p);
};
const pathKey = (p) => String(p).replace(/[\\/]+/g, "/").replace(/\/$/, "").toLowerCase();

function formatTokens(n) {
  if (n >= 1e9) return (n / 1e9).toFixed(2) + "B";
  if (n >= 1e6) return (n / 1e6).toFixed(2) + "M";
  if (n >= 1e4) return (n / 1e3).toFixed(1) + "k";
  return String(Math.round(n));
}
const formatUsd = (v) => (v > 0 && v < 0.01 ? "$" + v.toFixed(4) : "$" + v.toFixed(2));
const plural = (n, one, many) => (n === 1 ? "1 " + one : n + " " + many);

const TASK_STATES = {
  Queued: { label: "Na fila", tone: "queued" },
  Running: { label: "Rodando", tone: "run" },
  Completed: { label: "Concluída", tone: "ok" },
  Failed: { label: "Erro", tone: "err" },
  Cancelled: { label: "Cancelada", tone: "off" },
  Applied: { label: "Aplicada", tone: "ok" },
  Discarded: { label: "Descartada", tone: "off" },
};
const stateInfo = (s) => TASK_STATES[variant(s).kind] || { label: variant(s).kind || "?", tone: "off" };
const isActive = (s) => s === "Queued" || s === "Running";
const isClosed = (s) => s === "Applied" || s === "Discarded";

/** Ponto de status. Erro nunca depende só da cor: vira um ícone de alerta. */
function statusMark(tone, label) {
  if (tone === "err") {
    const el = h("span", { class: "mark err", role: "img", "aria-label": label || "Erro" });
    el.append(icon("alert", 16));
    return el;
  }
  return h("span", { class: "mark dot " + tone, role: "img", "aria-label": label || "" });
}

const PATHS_HEADER = "Arquivos anexados pelo caminho (abra com as ferramentas se precisar):";
/** Separa o texto do pedido do bloco de anexos que o desktop acrescenta. */
function splitPrompt(prompt) {
  const at = prompt.lastIndexOf(PATHS_HEADER);
  if (at < 0) return { text: prompt, attachments: [] };
  const attachments = [];
  for (const line of prompt.slice(at + PATHS_HEADER.length).split("\n")) {
    const m = /^- (.+?)(?: \(([^()]*)\))?$/.exec(line.trim());
    if (!m) {
      if (line.trim()) return { text: prompt, attachments: [] };
      continue;
    }
    attachments.push(baseName(m[1]));
  }
  return { text: prompt.slice(0, at).trimEnd(), attachments };
}

/** Título de uma sessão: a primeira linha do primeiro pedido. */
function sessionTitle(prompt, max) {
  const split = splitPrompt(String(prompt || ""));
  const text = split.text.trim() || String(prompt || "").trim();
  const first = (text.split(/\r?\n/).find((l) => l.trim()) || "").replace(/\s+/g, " ").trim();
  if (!first) return "(sem texto)";
  const chars = Array.from(first);
  const limit = max || 90;
  return chars.length > limit ? chars.slice(0, limit).join("") + "…" : first;
}

/** Aviso da CLI que cita a organização da conta: nunca é mostrado. */
function isHiddenNotice(line) {
  let text = String(line).trim();
  if (text.startsWith("[stderr]")) text = text.slice(8).trim();
  return /^warning:\s*claude\.ai mcp servers blocked/i.test(text);
}

/* ============================================================ markdown */
/* Mesmo conversor do desktop (src/views/chat/markdown.ts): tudo é escapado antes. */

function esc(s) {
  return String(s).replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]);
}
function mdInline(s) {
  const codes = [];
  let out = s.replace(/`([^`\n]+)`/g, (_, c) => {
    codes.push(c);
    return "\u0000" + (codes.length - 1) + "\u0000";
  });
  out = out
    .replace(/\*\*([^*\n]+)\*\*/g, "<b>$1</b>")
    .replace(/(^|[^\w*])\*([^*\n]+)\*(?=[^\w*]|$)/g, "$1<i>$2</i>")
    .replace(/(^|[^\w])_([^_\n]+)_(?=[^\w]|$)/g, "$1<i>$2</i>")
    .replace(/\[([^\]\n]+)\]\((https?:\/\/[^)\s]+)\)/g, '<a href="$2" target="_blank" rel="noopener noreferrer">$1</a>');
  return out.replace(/\u0000(\d+)\u0000/g, (_, i) => "<code>" + codes[Number(i)] + "</code>");
}
const TABLE_SEP = /^\s*\|?\s*:?-{2,}:?\s*(\|\s*:?-{2,}:?\s*)*\|?\s*$/;
const IS_ITEM = /^\s*([-*+]|\d+[.)])\s+/;
const mdCells = (line) => line.trim().replace(/^\|/, "").replace(/\|$/, "").split("|").map((c) => mdInline(c.trim()));

function mdList(lines) {
  let html = "";
  const stack = [];
  for (const line of lines) {
    const m = /^(\s*)([-*+]|\d+[.)])\s+([\s\S]*)$/.exec(line);
    const indent = m[1].replace(/\t/g, "  ").length;
    const tag = /\d/.test(m[2]) ? "ol" : "ul";
    while (stack.length && indent < stack[stack.length - 1].indent) html += "</li></" + stack.pop().tag + ">";
    const top = stack[stack.length - 1];
    if (!top || indent > top.indent) {
      html += "<" + tag + ">";
      stack.push({ indent, tag });
    } else {
      html += "</li>";
    }
    const parts = m[3].split("\n");
    let body = mdInline(parts[0]);
    const quote = [];
    for (const extra of parts.slice(1)) {
      if (/^&gt;\s?/.test(extra)) quote.push(mdInline(extra.replace(/^&gt;\s?/, "")));
      else body += " " + mdInline(extra);
    }
    if (quote.length) body += "<blockquote>" + quote.join("<br>") + "</blockquote>";
    html += "<li>" + body;
  }
  while (stack.length) html += "</li></" + stack.pop().tag + ">";
  return html;
}

function mdBlocks(text) {
  const lines = esc(text).split("\n");
  const out = [];
  let para = [];
  const flush = () => {
    if (para.length) out.push("<p>" + para.map(mdInline).join("<br>") + "</p>");
    para = [];
  };
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    if (!line.trim()) {
      flush();
      continue;
    }
    const head = /^(#{1,6})\s+(.*)$/.exec(line);
    if (head) {
      flush();
      const level = Math.min(6, head[1].length + 2);
      out.push("<h" + level + ">" + mdInline(head[2]) + "</h" + level + ">");
      continue;
    }
    if (/^\s*(-{3,}|\*{3,}|_{3,})\s*$/.test(line)) {
      flush();
      out.push("<hr>");
      continue;
    }
    if (line.includes("|") && i + 1 < lines.length && TABLE_SEP.test(lines[i + 1])) {
      flush();
      const headCells = mdCells(line);
      let j = i + 2;
      const rows = [];
      while (j < lines.length && lines[j].includes("|") && lines[j].trim()) rows.push(mdCells(lines[j++]));
      out.push(
        '<div class="md-table" tabindex="0" role="group" aria-label="Tabela (role para o lado)"><table><thead><tr>' +
          headCells.map((c) => "<th>" + c + "</th>").join("") +
          "</tr></thead><tbody>" +
          rows.map((r) => "<tr>" + r.map((c) => "<td>" + c + "</td>").join("") + "</tr>").join("") +
          "</tbody></table></div>",
      );
      i = j - 1;
      continue;
    }
    if (/^&gt;\s?/.test(line)) {
      flush();
      const quote = [];
      while (i < lines.length && /^&gt;\s?/.test(lines[i])) quote.push(lines[i++].replace(/^&gt;\s?/, ""));
      i--;
      out.push("<blockquote>" + quote.map(mdInline).join("<br>") + "</blockquote>");
      continue;
    }
    if (IS_ITEM.test(line)) {
      flush();
      const items = [];
      while (i < lines.length && (IS_ITEM.test(lines[i]) || (items.length && /^\s{2,}\S/.test(lines[i])))) {
        if (IS_ITEM.test(lines[i])) items.push(lines[i]);
        else items[items.length - 1] += "\n" + lines[i].trim();
        i++;
      }
      i--;
      out.push(mdList(items));
      continue;
    }
    para.push(line);
  }
  flush();
  return out.join("");
}

const MD_LANG = /^[\w+#.-]{1,24}$/;
function mdCode(part) {
  const nl = part.indexOf("\n");
  const info = nl >= 0 ? part.slice(0, nl).trim() : "";
  const code = nl >= 0 ? part.slice(nl + 1) : part;
  const lang = MD_LANG.test(info) ? info : "";
  return (
    '<div class="md-code"><div class="md-code-head"><span class="md-code-lang">' +
    esc(lang || "código") +
    '</span><button type="button" class="md-copy">Copiar</button></div><pre><code>' +
    esc(code.replace(/\n$/, "")) +
    "</code></pre></div>"
  );
}
const mdCache = new Map();
/** Markdown em HTML seguro (nenhum HTML do texto chega à página). */
function md(src) {
  const key = String(src);
  let html = mdCache.get(key);
  if (html == null) {
    html = key
      .split(/```/)
      .map((part, i) => (i % 2 === 1 ? mdCode(part) : mdBlocks(part)))
      .join("");
    if (mdCache.size > 400) mdCache.clear();
    mdCache.set(key, html);
  }
  return html;
}

async function copyText(text) {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch (_) {
    // http na rede local não é contexto seguro: cai no jeito antigo
    const ta = h("textarea", { class: "offscreen", readonly: true });
    ta.value = text;
    document.body.append(ta);
    ta.select();
    let ok = false;
    try {
      ok = document.execCommand("copy");
    } catch (_e) {
      ok = false;
    }
    ta.remove();
    return ok;
  }
}
/* ============================================================ anexos (galeria do aparelho)

   O aparelho escolhe imagens na galeria (ou na câmera), reduz o tamanho e as
   envia ao computador por POST /api/uploads. O arquivo fica na pasta de anexos
   do Argus; o pedido só leva o CAMINHO, no mesmo bloco que o desktop monta, e
   o agente recebe a liberação de leitura daquela pasta. */

/** Limite por anexo (o mesmo do computador). */
const MAX_UPLOAD = 8 * 1024 * 1024;
/** Maior lado de uma foto enviada: acima disso a imagem é reduzida no aparelho. */
const MAX_SIDE = 1600;

/** Id de rascunho da conversa nova: a pasta de anexos só dela. */
function draftFor(key) {
  const id = loadKey("argus.anexos." + key, "");
  if (id) return id;
  const novo = "cel" + Math.random().toString(36).slice(2, 10);
  saveKey("argus.anexos." + key, novo);
  return novo;
}

/** Foto da galeria reduzida a `MAX_SIDE` (JPEG); o que já é pequeno vai como está. */
async function shrinkImage(file) {
  const keep = file.size <= 400 * 1024 && /^image\/(png|jpeg|webp|gif)$/.test(file.type);
  if (keep) return { blob: file, name: file.name || "imagem.png" };
  let bmp;
  try {
    bmp = await createImageBitmap(file);
  } catch (_) {
    return { blob: file, name: file.name || "imagem.png" };
  }
  const scale = Math.min(1, MAX_SIDE / Math.max(bmp.width, bmp.height));
  const c = document.createElement("canvas");
  c.width = Math.max(1, Math.round(bmp.width * scale));
  c.height = Math.max(1, Math.round(bmp.height * scale));
  c.getContext("2d").drawImage(bmp, 0, 0, c.width, c.height);
  bmp.close();
  const blob = await new Promise((res) => c.toBlob(res, "image/jpeg", 0.85));
  c.width = c.height = 0;
  if (!blob) return { blob: file, name: file.name || "imagem.png" };
  const base = (file.name || "foto").replace(/\.\w+$/, "");
  return { blob, name: base + ".jpg" };
}

function toBase64(blob) {
  return new Promise((res, rej) => {
    const r = new FileReader();
    r.onload = () => res(String(r.result).split(",")[1] || "");
    r.onerror = () => rej(new Error("não foi possível ler o arquivo"));
    r.readAsDataURL(blob);
  });
}

/**
 * Faixa de anexos de uma caixa de composição. `target()` devolve
 * `{ task }` (conversa existente) ou `{ draft }` (conversa nova).
 */
function attachStrip(target, onChange) {
  const items = [];
  const strip = h("div", { class: "atts-edit", hidden: true });
  const file = h("input", { type: "file", accept: "image/*", multiple: true, class: "offscreen", "aria-hidden": "true", tabindex: "-1" });
  const button = h("button", { type: "button", class: "clip", "aria-label": "Anexar imagens da galeria", title: "Anexar imagens da galeria" }, icon("image", 22));
  button.addEventListener("click", () => file.click());
  file.addEventListener("change", () => {
    add(Array.from(file.files || []));
    file.value = "";
  });

  function paint() {
    strip.hidden = items.length === 0;
    strip.replaceChildren(
      ...items.map((it) =>
        h(
          "span",
          { class: "att-edit " + it.status, title: it.error || it.name },
          it.url ? h("img", { src: it.url, alt: "", class: "att-thumb" }) : icon("file", 16),
          h("span", { class: "att-name", text: it.name }),
          it.status === "saving" ? h("span", { class: "spinner tiny" }) : null,
          it.status === "error" ? icon("alert", 14) : null,
          h(
            "button",
            {
              type: "button",
              class: "att-x",
              "aria-label": "Tirar " + it.name,
              onclick: () => {
                const at = items.indexOf(it);
                if (at >= 0) items.splice(at, 1);
                if (it.url) URL.revokeObjectURL(it.url);
                paint();
                if (onChange) onChange();
              },
            },
            icon("close", 14),
          ),
        ),
      ),
    );
  }

  async function add(files) {
    for (const f of files.slice(0, 10)) {
      if (!f.type.startsWith("image/")) {
        toast("Só dá para anexar imagens por aqui: " + (f.name || "arquivo"), "err");
        continue;
      }
      const it = { name: f.name || "imagem", status: "saving", url: null, path: null, error: "" };
      items.push(it);
      paint();
      if (onChange) onChange();
      try {
        const { blob, name } = await shrinkImage(f);
        if (blob.size > MAX_UPLOAD) throw new Error("acima de " + MAX_UPLOAD / 1024 / 1024 + " MB");
        it.name = name;
        it.url = URL.createObjectURL(blob);
        const where = target();
        const res = await post("/api/uploads", {
          task: where.task == null ? null : where.task,
          draft: where.draft || null,
          name,
          data: await toBase64(blob),
        });
        it.path = res.path;
        it.status = "ready";
      } catch (err) {
        it.status = "error";
        it.error = err.message;
        toast("Não foi possível anexar " + it.name + ": " + err.message, "err");
      }
      paint();
      if (onChange) onChange();
    }
  }

  return {
    strip,
    button,
    file,
    /** Algum anexo ainda está subindo (o envio espera). */
    get busy() {
      return items.some((i) => i.status === "saving");
    },
    get count() {
      return items.length;
    },
    /** O bloco de caminhos que vai no fim do pedido ("" se não houver anexo pronto). */
    block() {
      const ready = items.filter((i) => i.status === "ready" && i.path);
      if (!ready.length) return "";
      return "\n\n" + PATHS_HEADER + "\n" + ready.map((i) => "- " + i.path + " (imagem)").join("\n");
    },
    clear() {
      for (const it of items) if (it.url) URL.revokeObjectURL(it.url);
      items.length = 0;
      paint();
    },
  };
}

document.addEventListener("click", (e) => {
  const btn = e.target.closest && e.target.closest("button.md-copy");
  if (!btn) return;
  e.preventDefault();
  const pre = btn.closest(".md-code").querySelector("pre");
  copyText(pre ? pre.textContent : "").then((ok) => {
    btn.textContent = ok ? "Copiado" : "Falhou";
    btn.classList.toggle("done", ok);
    setTimeout(() => {
      btn.textContent = "Copiar";
      btn.classList.remove("done");
    }, 1400);
  });
});

/* ============================================================ eventos da conversa */
/* Mesmas regras de src/views/session/events.ts. */

function unwrap(event) {
  let owner = null;
  let cur = event;
  while (cur && typeof cur === "object" && "Scoped" in cur) {
    owner = cur.Scoped.subagent;
    cur = cur.Scoped.event;
  }
  return { owner, event: cur };
}
const SPAWN_TOOLS = new Set(["agent", "task"]);

function pushNotice(out, line) {
  const text = String(line)
    .split(/\r?\n/)
    .filter((l) => !isHiddenNotice(l))
    .join("\n")
    .trim();
  if (!text) return;
  const last = out[out.length - 1];
  if (last && last.type === "notices") last.lines.push(text);
  else out.push({ type: "notices", lines: [text] });
}

/** Itens do chat de `owner` (null = agente principal). */
function chatEntries(events, owner) {
  const out = [];
  const spawnTools = [];
  /** Ferramentas ainda sem saída, em ordem: o próximo `ToolResult` é da primeira. */
  const pending = [];
  for (const raw of events) {
    const u = unwrap(raw);
    const event = u.event;
    if (!event || typeof event !== "object") continue;
    if ("SubagentStart" in event) {
      if ((event.SubagentStart.parent || null) !== owner) continue;
      const card = { type: "subagent", id: event.SubagentStart.id };
      const at = spawnTools.shift();
      if (at != null) out[at] = card;
      else out.push(card);
      continue;
    }
    if ("SubagentEnd" in event) continue;
    if (u.owner !== owner) continue;
    // Um turno novo descarta saídas que nunca vieram (turno interrompido).
    if ("User" in event) pending.length = 0;
    if ("User" in event) out.push({ type: "user", text: String(event.User) });
    else if ("Text" in event) {
      if (String(event.Text).trim()) out.push({ type: "text", text: String(event.Text) });
    } else if ("Thinking" in event) {
      if (String(event.Thinking).trim()) out.push({ type: "thinking", text: String(event.Thinking) });
    } else if ("ToolUse" in event) {
      const tool = { type: "tool", name: String(event.ToolUse.name), input: event.ToolUse.input, result: null };
      if (SPAWN_TOOLS.has(tool.name.toLowerCase())) spawnTools.push(out.length);
      else pending.push(tool);
      out.push(tool);
    } else if ("ToolResult" in event) {
      const tool = pending.shift();
      if (tool) tool.result = { ok: !!event.ToolResult.ok, output: String(event.ToolResult.output || "") };
    } else if ("Result" in event) out.push({ type: "result", ok: !!event.Result.ok, summary: String(event.Result.summary || "") });
    else if ("Usage" in event) out.push({ type: "usage", usage: event.Usage });
    else if ("Raw" in event) pushNotice(out, event.Raw);
    else if ("Denied" in event) out.push({ type: "denied", tool: String(event.Denied.tool), command: String(event.Denied.command || "") });
    else if ("ProviderSwitch" in event) {
      // Trocas seguidas sem mensagem no meio viram uma divisória só.
      const last = out[out.length - 1];
      const sw = event.ProviderSwitch;
      if (last && last.type === "switch") {
        if (last.from === sw.to) out.pop();
        else last.to = sw.to;
      } else out.push({ type: "switch", from: String(sw.from), to: String(sw.to) });
    }
  }
  return out;
}

/** Subagentes em ordem de árvore (pai antes dos filhos), com a profundidade. */
function subagentTree(events) {
  const flat = [];
  const byId = new Map();
  for (const raw of events) {
    const event = unwrap(raw).event;
    if (!event || typeof event !== "object") continue;
    if ("SubagentStart" in event) {
      const s = event.SubagentStart;
      if (byId.has(s.id)) continue;
      const node = { id: s.id, parent: s.parent || null, description: s.description || "", subagent_type: s.subagent_type || "", prompt: s.prompt || "", done: false, ok: null, result: null, depth: 0 };
      byId.set(s.id, node);
      flat.push(node);
    } else if ("SubagentEnd" in event) {
      const node = byId.get(event.SubagentEnd.id);
      if (node) {
        node.done = true;
        node.ok = !!event.SubagentEnd.ok;
        node.result = String(event.SubagentEnd.result || "");
      }
    }
  }
  const out = [];
  const placed = new Set();
  const visit = (parent, depth) => {
    for (const node of flat) {
      if (node.parent !== parent || placed.has(node)) continue;
      node.depth = depth;
      placed.add(node);
      out.push(node);
      visit(node.id, depth + 1);
    }
  };
  visit(null, 0);
  for (const node of flat) if (!placed.has(node)) out.push(node);
  return out;
}
function subagentStatus(node, taskActive) {
  if (node.done) return node.ok === false ? { tone: "err", label: "falhou" } : { tone: "ok", label: "concluído" };
  return taskActive ? { tone: "run", label: "rodando" } : { tone: "off", label: "interrompido" };
}

function priceFor(pricing, model) {
  if (!model) return pricing.fallback;
  const m = String(model).toLowerCase();
  let best = null;
  for (const key of Object.keys(pricing.models || {})) {
    if (m.includes(key.toLowerCase()) && (best == null || key.length > best.length)) best = key;
  }
  return best ? pricing.models[best] : pricing.fallback;
}
/** Custo equivalente em API (USD) de um uso; mesma regra do core. */
function usageCost(u, pricing) {
  if (pricing.prefer_reported && u.cost_usd != null && u.cost_usd > 0) return u.cost_usd;
  const p = priceFor(pricing, u.model);
  return (u.input_tokens * p.input + u.output_tokens * p.output + u.cache_read * p.cache_read + u.cache_write * p.cache_write) / 1e6;
}
/** Soma dos `Usage` do agente principal (já inclui os subagentes). */
function usageTotals(events, pricing) {
  const t = { tokens: 0, cost: pricing ? 0 : null, turns: 0 };
  for (const event of events) {
    if (!event || typeof event !== "object" || !("Usage" in event)) continue;
    const u = event.Usage;
    t.tokens += u.input_tokens + u.output_tokens + u.cache_read + u.cache_write;
    t.turns += 1;
    if (pricing) t.cost += usageCost(u, pricing);
  }
  return t;
}

function splitTurns(entries) {
  const turns = [];
  for (const entry of entries) {
    if (entry.type === "user") turns.push({ user: entry.text, items: [] });
    // A divisória de troca de agente fica entre os turnos, fora de "N passos".
    else if (entry.type === "switch") turns.push({ user: null, items: [entry] }, { user: null, items: [] });
    else {
      if (!turns.length) turns.push({ user: null, items: [] });
      turns[turns.length - 1].items.push(entry);
    }
  }
  return turns.filter((t) => t.user != null || t.items.length > 0);
}
/** Índice da resposta final do turno; o que vem antes vira "N passos". */
function stepsCut(items) {
  let last = -1;
  for (let i = items.length - 1; i >= 0; i--) {
    if (items[i].type === "text") {
      last = i;
      break;
    }
  }
  if (last <= 0) return -1;
  return items.slice(0, last).some((e) => e.type === "tool" || e.type === "subagent") ? last : -1;
}
/** Passos à vista enquanto o turno roda; -1 enquanto couber tudo. */
const LIVE_KEEP = 8;
function liveCut(items) {
  return items.length > LIVE_KEEP + 2 ? items.length - LIVE_KEEP : -1;
}
function stepsLabel(items) {
  const steps = items.filter((e) => e.type === "tool" || e.type === "text" || e.type === "subagent").length;
  const subs = items.filter((e) => e.type === "subagent").length;
  const label = plural(steps, "passo", "passos");
  return subs > 0 ? label + " · " + plural(subs, "subagente", "subagentes") : label;
}
/** Caminho comprido encurtado pelo meio: na tela do celular, o fim é o que importa. */
function shortPath(path, max) {
  const limit = max || 38;
  if (path.length <= limit) return path;
  const sep = path.includes("\\") && !path.includes("/") ? "\\" : "/";
  const parts = path.split(/[\\/]/).filter(Boolean);
  for (let keep = 3; keep >= 1; keep--) {
    if (parts.length <= keep) break;
    const tail = "…" + sep + parts.slice(-keep).join(sep);
    if (tail.length <= limit) return tail;
  }
  const last = parts.length ? parts[parts.length - 1] : path;
  return last.length <= limit ? "…" + sep + last : last.slice(0, limit - 12) + "…" + last.slice(-9);
}

/** Resumo de uma linha da entrada, por ferramenta (o mesmo do computador). */
function toolSummary(name, input) {
  if (input == null) return "";
  if (typeof input !== "object") return String(input);
  const str = (...keys) => {
    for (const k of keys) {
      const v = input[k];
      if (typeof v === "string" && v.trim()) return v.trim();
    }
    return "";
  };
  const t = String(name || "").toLowerCase();
  const path = () => shortPath(str("file_path", "path", "filePath", "notebook_path"));
  if (t === "read") {
    const off = typeof input.offset === "number" ? input.offset : null;
    const lim = typeof input.limit === "number" ? input.limit : null;
    if (off == null && lim == null) return path();
    const from = off == null ? 1 : off;
    return lim != null ? path() + " (linhas " + from + "–" + (from + lim - 1) + ")" : path() + " (a partir da linha " + from + ")";
  }
  if (t === "grep" || t === "glob") {
    const where = shortPath(str("path", "glob"));
    return where ? str("pattern") + "  em " + where : str("pattern");
  }
  const first = str("command", "pattern", "query", "url", "description", "prompt");
  if (first) return first.split("\n")[0];
  return path();
}

/** O que o agente está fazendo agora, em uma linha (o mesmo do computador). */
function liveActivity(entries) {
  for (let i = entries.length - 1; i >= 0; i--) {
    const e = entries[i];
    if (e.type === "user") break;
    if (e.type === "tool") {
      if (e.result) break;
      const sum = toolSummary(e.name, e.input);
      return sum ? e.name + " · " + sum : e.name;
    }
    if (e.type === "thinking") return "pensando…";
    if (e.type === "text") return "escrevendo a resposta…";
    if (e.type === "subagent") return "esperando o subagente…";
  }
  return "trabalhando…";
}

/** Texto do bloco "comando rodado": o comando do shell, ou "chave: valor" por linha. */
function toolInputText(name, input) {
  if (input == null) return "";
  if (typeof input !== "object") return String(input);
  const cmd = input.command;
  if (typeof cmd === "string" && cmd.trim() && /bash|shell|powershell/i.test(String(name))) return cmd;
  return Object.keys(input)
    .map((k) => k + ": " + (typeof input[k] === "string" ? input[k] : JSON.stringify(input[k], null, 2)))
    .join("\n");
}

/* ============================================================ estado e API */

const S = {
  tasks: null, // null = ainda não carregou
  tasksError: null,
  projects: [],
  providers: [],
  pricing: null,
  servers: [],
  serversError: null,
  aw: { available: true, loaded: false, error: null, settings: {}, runs: {} },
  conn: "connecting", // connecting | on | off
  unread: loadJson("argus.naolidas", {}),
  collapsed: loadJson("argus.recolhidos", []),
  showClosed: false,
  search: "",
};
/** Conversa aberta (a mesma lista de eventos serve o chat principal e os subagentes). */
const chat = { id: null, events: [], loaded: false, loading: false, error: null, dirty: false, open: new Set(), fresh: 0 };

let gated = false; // acesso revogado: para tudo

async function api(path, opts) {
  let r;
  try {
    const init = Object.assign({ credentials: "same-origin" }, opts || {});
    if (init.body) init.headers = { "Content-Type": "application/json" };
    r = await fetch(path, init);
  } catch (_) {
    throw new Error("Sem conexão com o computador.");
  }
  if (r.status === 401) {
    showGate();
    throw new Error("Não autorizado.");
  }
  const txt = await r.text();
  let data = null;
  try {
    data = txt ? JSON.parse(txt) : null;
  } catch (_) {
    data = null;
  }
  if (!r.ok) throw new Error((data && data.error) || "Erro " + r.status + " do servidor.");
  return data;
}
const post = (path, body) => api(path, { method: "POST", body: body == null ? undefined : JSON.stringify(body) });

const taskById = (id) => (S.tasks || []).find((t) => t.id === id) || null;

async function loadTasks() {
  try {
    S.tasks = await api("/api/tasks");
    S.tasksError = null;
  } catch (e) {
    S.tasksError = e.message;
    if (S.tasks == null) S.tasks = null;
  }
  changed();
}
async function loadProjects() {
  try {
    S.projects = await api("/api/projects");
  } catch (_) {
    /* a lista de sessões já mostra o erro */
  }
  changed();
}
async function loadStatic() {
  try {
    const r = await Promise.all([api("/api/providers"), api("/api/pricing")]);
    S.providers = r[0] || [];
    S.pricing = r[1];
  } catch (_) {
    /* sem tabela de preços: o custo não aparece */
  }
  changed();
}
async function loadAutowork() {
  try {
    const d = await api("/api/autowork");
    S.aw.settings = {};
    for (const x of d.settings || []) S.aw.settings[pathKey(x.project)] = x.settings;
    S.aw.runs = {};
    for (const r of d.runs || []) S.aw.runs[r.task] = r;
    S.aw.available = true;
    S.aw.error = null;
  } catch (e) {
    if (/indispon/i.test(e.message)) S.aw.available = false;
    else S.aw.error = e.message;
  }
  S.aw.loaded = true;
  changed();
}
async function loadServers() {
  try {
    S.servers = (await api("/api/servers")) || [];
    S.serversError = null;
  } catch (e) {
    S.serversError = e.message;
  }
  S.serversLoaded = true;
  changed();
}
function refreshAll() {
  if (gated) return;
  loadTasks();
  loadProjects();
  loadAutowork();
  loadServers();
  if (!S.pricing) loadStatic();
  if (chat.id != null) loadChat(chat.id, true);
}

function markUnread(id, on) {
  if (on) S.unread[id] = 1;
  else delete S.unread[id];
  saveKey("argus.naolidas", JSON.stringify(S.unread));
}

async function loadChat(id, quiet) {
  if (chat.id !== id) {
    chat.id = id;
    chat.events = [];
    chat.loaded = false;
    chat.open = new Set();
    chat.fresh = 0;
  }
  if (chat.loading) {
    chat.dirty = true;
    return;
  }
  chat.loading = true;
  chat.dirty = false;
  if (!quiet) chat.error = null;
  changed();
  try {
    const d = await api("/api/tasks/" + id);
    if (chat.id !== id) return;
    chat.events = d.events || [];
    chat.loaded = true;
    chat.error = null;
    const i = (S.tasks || []).findIndex((t) => t.id === id);
    if (S.tasks && i >= 0) S.tasks[i] = d.task;
    else if (S.tasks) S.tasks.push(d.task);
    else S.tasks = [d.task];
  } catch (e) {
    if (chat.id === id && !chat.loaded) chat.error = e.message;
  } finally {
    chat.loading = false;
    if (chat.id === id && chat.dirty) loadChat(id, true);
    changed();
  }
}

/** Ocupação da janela de contexto para o subtítulo da conversa (vazio sem dado). */
function contextInfo(c) {
  if (!c || typeof c !== "object") return [];
  const total = (Number(c.input_tokens) || 0) + (Number(c.cache_read) || 0) + (Number(c.cache_write) || 0);
  const win = Number(c.window) || 0;
  const n = (v) => Number(v).toLocaleString("pt-BR");
  if (!win) return [" · contexto " + n(total) + " tokens"];
  const pct = Math.min(100, Math.round((total / win) * 100));
  const level = pct >= 95 ? "err" : pct >= 80 ? "warn" : "";
  const text = "contexto " + pct + "%" + (level === "err" ? " (quase cheio)" : level === "warn" ? " (acima de 80%)" : "");
  return [" · ", h("span", { class: "ctx " + level, title: n(total) + " de " + n(win) + " tokens" + (c.model ? " · " + c.model : "") }, text)];
}

/* ============================================================ WebSocket */

let ws = null;
let wsTimer = 0;
let wsAttempt = 0;
let everOn = false;
let bannerTimer = 0;
let lastBeat = 0; // último sinal de vida do servidor (ele manda um a cada 5 s)

function setConn(state) {
  const before = S.conn;
  S.conn = state;
  const el = $("conn");
  if (!el) return;
  clearTimeout(bannerTimer);
  if (state === "off") {
    el.className = "conn lost";
    el.replaceChildren(icon("wifiOff", 18), h("span", { text: "Conexão perdida. Reconectando…" }));
    el.hidden = false;
  } else if (state === "on" && before === "off") {
    el.className = "conn back";
    el.replaceChildren(icon("check", 18), h("span", { text: "Conexão restabelecida." }));
    el.hidden = false;
    bannerTimer = setTimeout(() => {
      el.hidden = true;
    }, 2600);
  } else if (state === "on") {
    el.hidden = true;
  }
  document.documentElement.dataset.conn = state;
  changed();
}

function connect() {
  clearTimeout(wsTimer);
  if (gated) return;
  if (ws && (ws.readyState === 0 || ws.readyState === 1)) return;
  let sock;
  try {
    sock = new WebSocket((location.protocol === "https:" ? "wss://" : "ws://") + location.host + "/api/stream");
  } catch (_) {
    retry();
    return;
  }
  ws = sock;
  sock.onopen = () => {
    if (ws !== sock) return;
    lastBeat = Date.now();
    wsAttempt = 0;
    const again = everOn;
    everOn = true;
    setConn("on");
    // Tudo o que aconteceu durante a queda vem de uma recarga.
    if (again) refreshAll();
  };
  sock.onmessage = (m) => {
    if (ws !== sock) return;
    lastBeat = Date.now();
    let e;
    try {
      e = JSON.parse(m.data);
    } catch (_) {
      return;
    }
    onEvent(e);
  };
  sock.onclose = () => dropped(sock);
  sock.onerror = () => {
    /* o onclose cuida da nova tentativa */
  };
}
/** A conexão `sock` acabou (fechada pelo servidor, ou muda há tempo demais). */
function dropped(sock) {
  {
    if (ws !== sock) return;
    ws = null;
    sock.onclose = null;
    sock.onmessage = null;
    try {
      sock.close();
    } catch (_) {
      /* já fechada */
    }
    if (gated) return;
    setConn("off");
    // Token revogado também fecha o socket: uma consulta simples mostra o 401.
    api("/api/providers").catch(() => {});
    retry();
  }
}
/** Rede que some sem avisar (Wi-Fi caiu) não fecha o socket: sem sinal de vida por 16 s, reconecta. */
function checkBeat() {
  if (ws && ws.readyState === 1 && Date.now() - lastBeat > 16000) dropped(ws);
}
function retry() {
  clearTimeout(wsTimer);
  const wait = Math.min(8000, 1000 * Math.pow(2, wsAttempt));
  wsAttempt += 1;
  wsTimer = setTimeout(connect, wait);
}
function reconnectNow() {
  wsAttempt = 0;
  if (!ws || ws.readyState > 1) connect();
}

function viewingTask(id) {
  const r = parseRoute();
  return r.task === id && document.visibilityState === "visible";
}

function onEvent(e) {
  switch (e.type) {
    case "ping":
      return;
    case "build":
      // Build terminado na IDE (ver `runs` no core): só um aviso, sem mudar a tela.
      toast(e.text || "Build terminado.", e.ok ? "ok" : "err");
      return;
    case "resync":
      refreshAll();
      return;
    case "removed": {
      // Sessão excluída no computador: sai da lista e, se estava aberta, volta ao início.
      const r = parseRoute();
      if (r.task === e.task) go("");
      refreshAll();
      return;
    }
    case "autowork":
      S.aw.runs[e.task] = e.autowork;
      changed();
      return;
    case "state": {
      const t = taskById(e.task);
      if (!t) {
        loadTasks();
        loadProjects();
        return;
      }
      t.state = e.state;
      if (!isActive(e.state) && !viewingTask(e.task)) markUnread(e.task, true);
      // O fim do turno muda sessão, worktree e servidores: recarrega os detalhes.
      if (!isActive(e.state)) {
        if (chat.id === e.task) loadChat(e.task, true);
        loadServers();
      }
      changed();
      return;
    }
    case "agent":
    case "stderr": {
      const ev = e.type === "agent" ? e.event : { Raw: e.line };
      if (e.type === "stderr" && isHiddenNotice(e.line)) return;
      // Ocupação da janela de contexto: só atualiza a tarefa (não entra no chat).
      if (ev && typeof ev === "object" && "Context" in ev) {
        const t = taskById(e.task);
        if (t) {
          t.context = ev.Context;
          changed();
        }
        return;
      }
      // Agente trocado no computador: o provedor e o modelo da sessão mudaram.
      if (ev && typeof ev === "object" && "ProviderSwitch" in ev) loadTasks();
      if (chat.id === e.task) {
        if (chat.loaded && !chat.loading) {
          chat.events.push(ev);
          chat.fresh += 1;
        } else chat.dirty = true;
      }
      if (!viewingTask(e.task)) markUnread(e.task, true);
      changed();
      return;
    }
    default:
  }
}

/* ============================================================ avisos, folha de confirmação, acesso revogado */

let toastTimer = 0;
function toast(text, tone) {
  const el = $("toast");
  if (!el) return;
  el.className = "show " + (tone || "");
  el.replaceChildren(tone === "err" ? icon("alert", 18) : tone === "ok" ? icon("check", 18) : "", h("span", { text }));
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => {
    el.className = "";
  }, tone === "err" ? 5000 : 2800);
}

let sheetClose = null;
/** Confirmação em folha, de baixo para cima. Resolve true só no botão de confirmar. */
function confirmSheet(opts) {
  return new Promise((resolve) => {
    const box = $("sheet");
    const done = (value) => {
      box.hidden = true;
      box.replaceChildren();
      document.removeEventListener("keydown", onKey);
      sheetClose = null;
      resolve(value);
    };
    const onKey = (e) => {
      if (e.key === "Escape") done(false);
    };
    sheetClose = () => done(false);
    const cancel = h("button", { type: "button", class: "btn", onclick: () => done(false), text: "Cancelar" });
    const ok = h("button", { type: "button", class: "btn " + (opts.danger ? "danger" : "primary"), onclick: () => done(true), text: opts.ok || "Confirmar" });
    box.replaceChildren(
      h("div", { class: "scrim", onclick: () => done(false) }),
      h(
        "div",
        { class: "sheet-box", role: "alertdialog", "aria-modal": "true", "aria-labelledby": "sheet-title", "aria-describedby": "sheet-text" },
        h("h2", { id: "sheet-title", text: opts.title }),
        h("p", { id: "sheet-text", text: opts.text }),
        opts.detail ? h("pre", { class: "sheet-detail", text: opts.detail }) : null,
        h("div", { class: "sheet-actions" }, cancel, ok),
      ),
    );
    box.hidden = false;
    document.addEventListener("keydown", onKey);
    cancel.focus();
  });
}

/** Tela cheia de "não autorizado" (token revogado com o app aberto). Não tenta de novo sozinha. */
function showGate() {
  if (gated) return;
  gated = true;
  clearTimeout(wsTimer);
  try {
    if (ws) ws.close();
  } catch (_) {
    /* já fechado */
  }
  const app = $("app");
  if (!app) return;
  if (sheetClose) sheetClose();
  app.className = "gate";
  app.replaceChildren(
    h(
      "div",
      { class: "gate-box", role: "alert" },
      icon("shield", 52, "gate-ic"),
      h("div", null, h("span", { class: "tag err" }, icon("alert", 14), "Erro 401")),
      h("h1", { text: "Não autorizado" }),
      h("p", { text: "Este aparelho não está pareado, ou o acesso foi revogado." }),
      h(
        "ol",
        null,
        h("li", null, "No computador, abra o Argus e vá em ", h("b", { text: "Remoto" }), "."),
        h("li", { text: "Confira se o servidor está ligado." }),
        h("li", { text: "Leia o QR code com a câmera deste aparelho." }),
      ),
    ),
  );
}

/* ============================================================ rotas */

function parseRoute() {
  const raw = location.hash.replace(/^#\/?/, "");
  const parts = raw.split("/").filter(Boolean).map((p) => {
    try {
      return decodeURIComponent(p);
    } catch (_) {
      return p;
    }
  });
  if (parts[0] === "s" && /^\d+$/.test(parts[1] || "")) {
    const task = Number(parts[1]);
    if (parts[2] === "sub") return parts[3] ? { name: "subchat", task, sub: parts[3] } : { name: "subs", task };
    return { name: "chat", task };
  }
  if (parts[0] === "nova") return { name: "new", project: parts[1] || null };
  if (parts[0] === "autowork") return { name: "autowork" };
  if (parts[0] === "servidores") return { name: "servers" };
  if (parts[0] === "ajustes") return { name: "settings" };
  return { name: "home" };
}
function go(path) {
  const target = "#/" + path;
  if (location.hash === target) renderRoute();
  else location.hash = target;
}
function back(fallback) {
  // Volta no histórico quando veio de dentro do app; senão vai para a tela de cima.
  if (navStack.length > 1) history.back();
  else go(fallback);
}
/** Telas visitadas nesta abertura, para o "voltar" saber se há para onde voltar. */
const navStack = [location.hash];
let navReplace = false;
function trackNav() {
  const now = location.hash;
  if (navReplace) navStack[navStack.length - 1] = now;
  else if (navStack.length > 1 && navStack[navStack.length - 2] === now) navStack.pop();
  else navStack.push(now);
  navReplace = false;
}

const VIEWS = {};
let current = null; // { name, key, update, unmount }
let pending = 0;

/** Algum dado mudou: atualiza a tela atual (no máximo uma vez por quadro). */
function changed() {
  if (pending || gated) return;
  pending = setTimeout(() => {
    pending = 0;
    if (gated || !current) return;
    try {
      current.update();
    } catch (e) {
      console.error(e);
    }
    renderTabs();
  }, 30);
}

function renderRoute() {
  if (gated) return;
  if (sheetClose) sheetClose();
  const r = parseRoute();
  const key = JSON.stringify(r);
  if (current && current.key === key) {
    changed();
    return;
  }
  if (current && current.unmount) current.unmount();
  const top = $("top");
  const view = $("view");
  const foot = $("foot");
  top.replaceChildren();
  view.replaceChildren();
  foot.replaceChildren();
  view.className = "";
  foot.className = "";
  view.scrollTop = 0;
  $("app").dataset.view = r.name;
  current = VIEWS[r.name](r, top, view, foot);
  current.key = key;
  current.name = r.name;
  current.update();
  renderTabs();
}

const TABS = [
  { name: "home", path: "", label: "Sessões", icon: "chat" },
  { name: "autowork", path: "autowork", label: "Auto-work", icon: "loop" },
  { name: "servers", path: "servidores", label: "Servidores", icon: "server" },
  { name: "settings", path: "ajustes", label: "Ajustes", icon: "gear" },
];
function renderTabs() {
  const nav = $("tabs");
  if (!nav || !current) return;
  const show = TABS.some((t) => t.name === current.name);
  nav.hidden = !show;
  if (!show) return;
  const counts = {
    home: Object.keys(S.unread).filter((id) => taskById(Number(id))).length,
    autowork: Object.values(S.aw.runs).filter((r) => variant(r.status).kind === "Running").length,
    servers: S.servers.length,
  };
  nav.replaceChildren(
    ...TABS.map((t) => {
      const n = counts[t.name] || 0;
      return h(
        "a",
        { class: "tab" + (t.name === current.name ? " on" : ""), href: "#/" + t.path, "aria-current": t.name === current.name ? "page" : null },
        h("span", { class: "tab-ic" }, icon(t.icon, 22), n > 0 ? h("span", { class: "tab-badge", text: n > 99 ? "99+" : String(n) }) : null),
        h("span", { class: "tab-label", text: t.label }),
      );
    }),
  );
}

/* Peças comuns */
function topBar(top, opts) {
  const bar = h("div", { class: "bar" });
  if (opts.back) bar.append(h("button", { type: "button", class: "ibtn", "aria-label": "Voltar", onclick: opts.back }, icon("back", 24)));
  else bar.append(h("img", { class: "logo", src: "/icon.svg?tema=" + document.documentElement.dataset.theme, alt: "", width: 30, height: 30 }));
  const title = h("div", { class: "bar-title" }, h("h1", { text: opts.title || "" }), h("div", { class: "bar-sub", text: opts.sub || "", hidden: !opts.sub }));
  bar.append(title);
  const right = h("div", { class: "bar-right" });
  bar.append(right);
  top.append(bar);
  return { bar, h1: title.firstChild, sub: title.lastChild, right };
}
function emptyBox(iconName, title, text, action) {
  return h("div", { class: "empty" }, icon(iconName, 34, "empty-ic"), h("b", { text: title }), text ? h("p", { text }) : null, action || null);
}
function loadingBox(text) {
  return h("div", { class: "empty loading", role: "status" }, h("span", { class: "spinner" }), h("p", { text: text || "Carregando…" }));
}
function errorBox(message, retryFn) {
  return h(
    "div",
    { class: "empty error", role: "alert" },
    icon("alert", 34, "empty-ic"),
    h("b", { text: "Erro ao carregar" }),
    h("p", { text: message }),
    retryFn ? h("button", { type: "button", class: "btn", onclick: retryFn }, icon("refresh", 18), "Tentar de novo") : null,
  );
}
function providerName(id) {
  const p = S.providers.find((x) => x.id === id);
  return p ? p.name : id;
}
function projectName(path) {
  const p = S.projects.find((x) => pathKey(x.path) === pathKey(path));
  return p ? p.name : baseName(path);
}
function awLabel(run) {
  const st = variant(run.status);
  if (st.kind === "Running") return "auto " + run.done + "/" + run.total;
  if (st.kind === "Paused") return "auto pausado";
  if (st.kind === "Completed") return "auto concluído";
  return "auto parado";
}
function awTone(run) {
  const k = variant(run.status).kind;
  return k === "Running" ? "run" : k === "Paused" ? "warn" : k === "Completed" ? "ok" : "off";
}
const serversOf = (taskId) => S.servers.filter((s) => s.owner && s.owner.kind === "Session" && s.owner.id === taskId);

/* ============================================================ tela: projetos e sessões */

VIEWS.home = (_r, top, view) => {
  const bar = topBar(top, { title: "Argus" });
  const live = h("span", { class: "live" });
  bar.right.append(
    live,
    h("a", { class: "ibtn accent", href: "#/nova", "aria-label": "Nova conversa", title: "Nova conversa" }, icon("plus", 24)),
  );
  const input = h("input", {
    type: "search",
    class: "search-input",
    placeholder: "Buscar projeto ou sessão…",
    "aria-label": "Buscar projeto ou sessão",
    autocomplete: "off",
    enterkeyhint: "search",
  });
  input.value = S.search;
  input.addEventListener("input", () => {
    S.search = input.value;
    update();
  });
  const closedBtn = h("button", { type: "button", class: "chip", onclick: () => {
    S.showClosed = !S.showClosed;
    update();
  } });
  top.append(h("div", { class: "search-row" }, h("label", { class: "search" }, icon("search", 18), input), closedBtn));
  const list = h("div", { class: "plist" });
  view.append(list);

  function sessionRow(t) {
    const info = stateInfo(t.state);
    const unread = !!S.unread[t.id];
    const run = S.aw.runs[t.id];
    const servers = serversOf(t.id).length;
    const kind = variant(t.state).kind;
    const meta = h("span", { class: "srow-meta" }, h("span", { class: "st " + info.tone, text: info.label }), h("span", { class: "sep", text: "·" }), h("span", { text: providerName(t.provider) }));
    const badges = h("span", { class: "srow-badges" });
    if (run) badges.append(h("span", { class: "badge " + awTone(run) }, icon("loop", 12), awLabel(run)));
    if (servers) badges.append(h("span", { class: "badge" }, icon("server", 12), plural(servers, "servidor", "servidores")));
    return h(
      "a",
      { class: "srow" + (unread ? " unread" : "") + (isClosed(kind) ? " closed" : ""), href: "#/s/" + t.id, "data-task": t.id },
      statusMark(info.tone, info.label),
      h(
        "span",
        { class: "srow-main" },
        h("span", { class: "srow-title", text: sessionTitle(t.prompt) }),
        h("span", { class: "srow-line" }, meta, badges.childNodes.length ? badges : null),
      ),
      unread ? h("span", { class: "new-dot", role: "img", "aria-label": "Com novidade" }) : null,
      icon("chevronRight", 18, "srow-chev"),
    );
  }

  function update() {
    live.className = "live " + S.conn;
    live.replaceChildren(h("span", { class: "live-dot" }), S.conn === "on" ? "ao vivo" : S.conn === "off" ? "sem conexão" : "conectando");
    const tasks = S.tasks || [];
    const closedCount = tasks.filter((t) => isClosed(variant(t.state).kind)).length;
    closedBtn.hidden = closedCount === 0;
    closedBtn.setAttribute("aria-pressed", String(S.showClosed));
    closedBtn.textContent = S.showClosed ? "Ocultar encerradas" : "Encerradas (" + closedCount + ")";

    if (S.tasks == null) {
      list.replaceChildren(S.tasksError ? errorBox(S.tasksError, refreshAll) : loadingBox("Carregando as sessões…"));
      return;
    }
    const q = S.search.trim().toLowerCase();
    // Projetos conhecidos mais os que só aparecem nas tarefas.
    const projects = S.projects.map((p) => ({ name: p.name, path: p.path }));
    for (const t of tasks) {
      if (!projects.some((p) => pathKey(p.path) === pathKey(t.project))) projects.push({ name: baseName(t.project), path: t.project });
    }
    projects.sort((a, b) => a.name.toLowerCase().localeCompare(b.name.toLowerCase()));
    if (!projects.length) {
      list.replaceChildren(
        S.tasksError
          ? errorBox(S.tasksError, refreshAll)
          : emptyBox("folder", "Nenhum projeto", "Abra uma pasta raiz no Argus do computador. Os projetos e as sessões aparecem aqui."),
      );
      return;
    }
    const nodes = [];
    if (S.tasksError) nodes.push(h("div", { class: "note err", role: "alert" }, icon("alert", 16), "Lista desatualizada: " + S.tasksError));
    let shown = 0;
    for (const p of projects) {
      const key = pathKey(p.path);
      let sessions = tasks.filter((t) => pathKey(t.project) === key).sort((a, b) => b.id - a.id);
      if (!S.showClosed) sessions = sessions.filter((t) => !isClosed(variant(t.state).kind));
      const nameHit = !q || p.name.toLowerCase().includes(q);
      if (q && !nameHit) {
        sessions = sessions.filter((t) => sessionTitle(t.prompt, 400).toLowerCase().includes(q) || t.provider.toLowerCase().includes(q) || ("#" + t.id).includes(q));
        if (!sessions.length) continue;
      }
      shown += 1;
      const collapsed = !q && S.collapsed.includes(key);
      const running = sessions.filter((t) => isActive(variant(t.state).kind)).length;
      const head = h(
        "div",
        { class: "phead" },
        h(
          "button",
          {
            type: "button",
            class: "phead-main",
            "aria-expanded": String(!collapsed),
            onclick: () => {
              S.collapsed = collapsed ? S.collapsed.filter((k) => k !== key) : S.collapsed.concat(key);
              saveKey("argus.recolhidos", JSON.stringify(S.collapsed));
              update();
            },
          },
          icon("chevronRight", 18, "phead-chev" + (collapsed ? "" : " open")),
          h("span", { class: "phead-name", text: p.name }),
          h("span", { class: "phead-count", text: running ? running + " rodando" : sessions.length ? plural(sessions.length, "sessão", "sessões") : "sem sessões" }),
        ),
        h("a", { class: "ibtn", href: "#/nova/" + encodeURIComponent(p.path), "aria-label": "Nova conversa em " + p.name, title: "Nova conversa em " + p.name }, icon("plus", 22)),
      );
      const group = h("section", { class: "pgroup" }, head);
      if (!collapsed) {
        if (sessions.length) group.append(h("div", { class: "slist" }, sessions.map(sessionRow)));
        else group.append(h("a", { class: "pnone", href: "#/nova/" + encodeURIComponent(p.path), text: "Nenhuma sessão ainda · começar uma conversa" }));
      }
      nodes.push(group);
    }
    if (!shown) nodes.push(emptyBox("search", "Nada encontrado", "Nenhum projeto ou sessão com “" + S.search.trim() + "”."));
    list.replaceChildren(...nodes);
  }
  return { update };
};

/* ============================================================ conversa: peças da transcrição */

function userMessage(text) {
  const split = splitPrompt(text);
  return h(
    "div",
    { class: "msg-user" },
    split.attachments.length
      ? h(
          "div",
          { class: "atts" },
          split.attachments.map((name) =>
            h("span", { class: "att" }, icon(/\.(png|jpe?g|gif|webp|bmp)$/i.test(name) ? "image" : "file", 14), name),
          ),
        )
      : null,
    split.text ? h("div", { class: "msg-user-text", text: split.text }) : null,
  );
}
function keepOpen(details, key) {
  if (chat.open.has(key)) details.open = true;
  // Anota no toque (antes de o navegador abrir): uma atualização da tela no meio não perde o estado.
  details.firstChild.addEventListener("click", () => {
    if (details.open) chat.open.delete(key);
    else chat.open.add(key);
  });
  return details;
}
const STATUS_TITLE = { run: "Rodando", ok: "Concluída", err: "Falhou", off: "Sem saída registrada" };

/** Bloco rotulado da ferramenta: o comando rodado e o resultado. */
function ioBlock(label, text, error) {
  return h("div", { class: "io" + (error ? " err" : "") }, h("span", { class: "io-label", text: label }), h("pre", { text: text || "(vazio)" }));
}

/** Lista de tarefas (TodoWrite) como checklist, igual à do computador. */
function todoList(input) {
  const todos = (input && input.todos) || [];
  if (!Array.isArray(todos) || !todos.length) return null;
  return h(
    "ul",
    { class: "todos" },
    todos.map((t) =>
      h(
        "li",
        { class: "todo " + (t.status || "") },
        icon(t.status === "completed" ? "check" : "chevronRight", 15),
        h("span", { text: t.status === "in_progress" && t.activeForm ? t.activeForm : t.content || "" }),
      ),
    ),
  );
}

function entryNode(entry, key, ctx) {
  switch (entry.type) {
    case "user":
      return userMessage(entry.text);
    case "text":
      return h("div", { class: "md", html: md(entry.text) });
    case "tool": {
      const isTodo = /todo/i.test(entry.name);
      const summary = isTodo ? "" : toolSummary(entry.name, entry.input);
      const result = entry.result || null;
      // Sem saída e com o turno rodando = ainda está rodando.
      const status = result ? (result.ok ? "ok" : "err") : ctx.live ? "run" : "off";
      const body = isTodo
        ? [todoList(entry.input)]
        : [
            ioBlock("Comando rodado", toolInputText(entry.name, entry.input), false),
            result && (result.output.trim() || !result.ok)
              ? ioBlock("Resultado", result.output.trim() || "(sem saída)", !result.ok)
              : h("div", { class: "io-note", text: result ? "(sem saída)" : ctx.live ? "Rodando…" : "Sem saída registrada." }),
          ];
      return keepOpen(
        h(
          "details",
          { class: "tool" + (isTodo ? " todo" : ""), open: isTodo },
          h(
            "summary",
            null,
            statusMark(status, STATUS_TITLE[status]),
            icon(toolIcon(entry.name), 16, "tool-ic"),
            h("b", { text: isTodo ? "Tarefas" : entry.name }),
            summary ? h("span", { class: "tool-sum", text: summary }) : null,
            icon("chevronRight", 16, "chev"),
          ),
          ...body,
        ),
        key,
      );
    }
    case "thinking":
      return keepOpen(
        h(
          "details",
          { class: "tool think" },
          h("summary", null, icon("bot", 16, "tool-ic"), h("span", { class: "think-label", text: ctx.live ? "Pensando…" : "Pensou" }), icon("chevronRight", 16, "chev")),
          h("div", { class: "think-text", text: entry.text }),
        ),
        key,
      );
    case "result":
      if (entry.ok) return ctx.showSummary && entry.summary.trim() ? h("div", { class: "md", html: md(entry.summary) }) : null;
      return h(
        "div",
        { class: "fail", role: "alert" },
        icon("alert", 18),
        h("span", null, h("b", { text: "Erro: o turno falhou." }), entry.summary.trim() ? h("span", { class: "fail-text", text: " " + entry.summary.trim() }) : null),
      );
    case "switch":
      return h("div", { class: "switch-line", role: "separator" }, h("span", { text: "Agente trocado: " + providerName(entry.from) + " → " + providerName(entry.to) }));
    case "denied": {
      const shell = entry.tool === "Bash" || entry.tool === "PowerShell";
      // Dá para liberar daqui o que o computador também libera com um toque:
      // comando de shell de uma linha, num projeto que a IDE conhece.
      const liberavel = shell && !!entry.command && !entry.command.includes("\n") && !!ctx.project;
      const porque = h("span", { class: "denied-why", text: "Não está entre os comandos liberados ao agente." });
      const box = h(
        "div",
        { class: "denied", role: "status" },
        icon("alert", 16),
        h(
          "span",
          null,
          h("b", { text: shell ? "Comando negado" : "Ferramenta negada: " + entry.tool }),
          entry.command ? h("code", { text: entry.command }) : null,
          porque,
        ),
      );
      if (!liberavel) {
        porque.textContent += " Libere no computador, na aba Configurações.";
        return box;
      }
      const botao = h("button", { type: "button", class: "btn small denied-allow" }, icon("shield", 16), "Liberar neste projeto");
      botao.addEventListener("click", async () => {
        botao.disabled = true;
        botao.lastChild.textContent = "Liberando…";
        try {
          await post("/api/permissions/allow", { project: ctx.project, tool: entry.tool, command: entry.command });
          botao.remove();
          porque.textContent = "Liberado neste projeto. Vale a partir do próximo turno: peça de novo.";
        } catch (e) {
          botao.disabled = false;
          botao.lastChild.textContent = "Liberar neste projeto";
          toast("Não foi possível liberar: " + e.message, "err");
        }
      });
      box.append(botao);
      return box;
    }
    case "usage": {
      const u = entry.usage;
      const parts = [formatTokens(u.input_tokens) + " entrada", formatTokens(u.output_tokens) + " saída", "cache " + formatTokens(u.cache_read) + " lido", formatTokens(u.cache_write) + " gravado"];
      if (u.model) parts.push(u.model);
      if (S.pricing) parts.push(formatUsd(usageCost(u, S.pricing)) + " equivalente em API");
      return h("div", { class: "usage", text: parts.join(" · ") });
    }
    case "notices": {
      const first = entry.lines[0].split(/\r?\n/)[0].replace(/^\[stderr\]\s*/, "");
      const cut = Array.from(first).slice(0, 80).join("");
      return keepOpen(
        h(
          "details",
          { class: "notice" },
          h("summary", null, icon("alert", 14), h("span", { class: "notice-sum", text: cut + (cut.length < first.length ? "…" : "") + (entry.lines.length > 1 ? " (+" + (entry.lines.length - 1) + ")" : "") }), icon("chevronRight", 16, "chev")),
          h("pre", { text: entry.lines.join("\n") }),
        ),
        key,
      );
    }
    case "subagent": {
      const node = ctx.subs.get(entry.id);
      if (!node) return null;
      const st = subagentStatus(node, ctx.active);
      return h(
        "a",
        { class: "sub-card", href: "#/s/" + ctx.task + "/sub/" + encodeURIComponent(node.id), "data-sub": node.id },
        icon("bot", 20, "sub-ic"),
        h(
          "span",
          { class: "sub-main" },
          h("span", { class: "sub-name" }, "Subagente ", h("b", { text: node.description || node.subagent_type || node.id })),
          h("span", { class: "sub-meta" }, node.subagent_type && node.description ? node.subagent_type + " · " : "", h("span", { class: "st " + st.tone }, st.tone === "err" ? icon("alert", 13) : null, st.label), " · somente leitura"),
        ),
        icon("chevronRight", 18, "srow-chev"),
      );
    }
    default:
      return null;
  }
}

/** Monta a transcrição de `owner` (null = agente principal) como lista de nós. */
function transcriptNodes(taskId, owner, active, group) {
  const entries = chatEntries(chat.events, owner);
  const subs = new Map(subagentTree(chat.events).map((n) => [n.id, n]));
  const turns = splitTurns(entries);
  const out = [];
  // Projeto da sessão: é dele a lista de comandos liberados ao agente.
  const tarefa = taskById(taskId);
  turns.forEach((turn, t) => {
    const finished = t < turns.length - 1 || !active;
    // Turno terminado: tudo antes da resposta vira "N passos". Em andamento:
    // só os últimos passos ficam à vista, o resto recolhe em "N passos antes".
    const cut = group ? (finished ? stepsCut(turn.items) : liveCut(turn.items)) : -1;
    const steps = cut > 0 ? turn.items.slice(0, cut) : [];
    const rest = cut > 0 ? turn.items.slice(cut) : turn.items;
    // O resumo do Result só aparece quando o turno não trouxe texto nenhum (ex.: comando da própria CLI).
    const ctx = {
      task: taskId,
      project: tarefa ? tarefa.project : null,
      subs,
      active,
      live: !finished,
      showSummary: !turn.items.some((e) => e.type === "text"),
    };
    const ctxDone = Object.assign({}, ctx, { live: false });
    const box = h("div", { class: "turn" });
    if (turn.user != null) box.append(userMessage(turn.user));
    if (steps.length) {
      const key = owner + ":" + t + ":steps";
      const body = h("div", { class: "steps-body" });
      steps.forEach((e, i) => {
        const n = entryNode(e, owner + ":" + t + ":s" + i, ctxDone);
        if (n) body.append(n);
      });
      const label = finished ? stepsLabel(steps) : stepsLabel(steps) + " antes";
      box.append(keepOpen(h("details", { class: "steps" }, h("summary", null, icon("chevronRight", 16, "chev"), label), body), key));
    }
    rest.forEach((e, i) => {
      const n = entryNode(e, owner + ":" + t + ":r" + i, ctx);
      if (n) box.append(n);
    });
    if (box.childNodes.length) out.push(box);
  });
  return out;
}

/** Troca o conteúdo da transcrição mantendo a leitura: só acompanha o fim se já estava no fim. */
function paintLog(view, log, nodes, force) {
  const stick = force || view.scrollHeight - view.scrollTop - view.clientHeight < 80;
  const before = view.scrollTop;
  log.replaceChildren(...nodes);
  view.scrollTop = stick ? view.scrollHeight : before;
  return stick;
}

/* ============================================================ tela: conversa */

VIEWS.chat = (r, top, view, foot) => {
  const id = r.task;
  const bar = topBar(top, { back: () => back(""), title: "Sessão #" + id });
  const pill = h("span", { class: "pill", hidden: true });
  bar.right.append(pill);
  const total = h("span", { class: "meta-total" });
  const chips = h("div", { class: "meta-chips" });
  top.append(h("div", { class: "meta-row" }, total, chips));

  const log = h("div", { class: "log" });
  const jump = h("button", { type: "button", class: "jump", hidden: true, onclick: () => {
    view.scrollTop = view.scrollHeight;
    jump.hidden = true;
  } }, icon("down", 18), "Novas mensagens");
  view.className = "chat";
  view.append(log, jump);
  const onScroll = () => {
    if (view.scrollHeight - view.scrollTop - view.clientHeight < 80) jump.hidden = true;
  };
  view.addEventListener("scroll", onScroll, { passive: true });

  // Caixa de composição: fica no rodapé do app, acima do teclado.
  const input = h("textarea", { class: "composer-input", rows: "1", placeholder: "Escreva para o agente…", "aria-label": "Mensagem para o agente", enterkeyhint: "enter" });
  input.value = loadKey("argus.rascunho." + id, "");
  const send = h("button", { type: "submit", class: "send" });
  const closedNote = h("div", { class: "readonly-bar", hidden: true });
  const atts = attachStrip(() => ({ task: id }), () => paintComposer());
  const form = h(
    "form",
    { class: "composer" },
    atts.strip,
    h("div", { class: "composer-box" }, atts.button, input, send, atts.file),
    closedNote,
  );
  foot.append(form);
  let sending = false;
  let first = true;
  let lastCount = -1;
  let painted = "";

  const grow = () => {
    input.style.height = "auto";
    input.style.height = Math.min(input.scrollHeight, 150) + "px";
  };
  input.addEventListener("input", () => {
    grow();
    saveKey("argus.rascunho." + id, input.value);
    paintComposer();
  });
  input.addEventListener("keydown", (e) => {
    if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      form.requestSubmit();
    }
  });
  input.addEventListener("focus", () => setTimeout(() => {
    view.scrollTop = view.scrollHeight;
  }, 250));

  function paintComposer() {
    const t = taskById(id);
    const kind = t ? variant(t.state).kind : "";
    const active = isActive(kind);
    const closed = isClosed(kind);
    const failed = !!chat.error && !chat.loaded;
    form.querySelector(".composer-box").hidden = closed || !t || failed;
    atts.strip.hidden = atts.count === 0 || closed || !t || failed;
    atts.button.disabled = active || sending;
    closedNote.hidden = !closed;
    form.hidden = !closed && (!t || failed);
    if (closed) closedNote.replaceChildren(icon("lock", 16), h("span", { text: "Sessão encerrada (" + stateInfo(t.state).label.toLowerCase() + "): não aceita novas mensagens." }));
    input.disabled = active || sending;
    input.placeholder = active ? "O agente está trabalhando…" : "Escreva para o agente…";
    send.className = "send" + (active ? " stop" : "");
    send.setAttribute("aria-label", active ? "Parar" : "Enviar");
    send.title = active ? "Parar o turno" : "Enviar";
    send.dataset.mode = active ? "stop" : "send";
    send.replaceChildren(active ? icon("stop", 20, null, true) : icon("send", 22), h("span", { class: "send-label", text: active ? "Parar" : "Enviar" }));
    send.disabled = sending || (!active && (atts.busy || (!input.value.trim() && !atts.count)));
  }

  form.addEventListener("submit", async (e) => {
    e.preventDefault();
    const t = taskById(id);
    if (!t || sending) return;
    const active = isActive(variant(t.state).kind);
    sending = true;
    paintComposer();
    try {
      if (active) {
        await post("/api/tasks/" + id + "/cancel");
        toast("Parando o turno…");
      } else {
        const message = input.value.trim() + atts.block();
        if (!message.trim()) return;
        await post("/api/tasks/" + id + "/reply", { message });
        atts.clear();
        input.value = "";
        saveKey("argus.rascunho." + id, "");
        grow();
        view.scrollTop = view.scrollHeight;
        // Sem WebSocket no momento, a resposta só chegaria na reconexão: recarrega já.
        if (S.conn !== "on") loadChat(id, true);
      }
    } catch (err) {
      toast((active ? "Não foi possível parar: " : "Não foi possível enviar: ") + err.message, "err");
    } finally {
      sending = false;
      paintComposer();
    }
  });

  function update() {
    const t = taskById(id);
    if (t) {
      const info = stateInfo(t.state);
      bar.h1.textContent = sessionTitle(t.prompt, 70);
      // Provedor e modelo atuais (mudam quando o agente é trocado no computador).
      bar.sub.replaceChildren(
        projectName(t.project) + " · " + providerName(t.provider) + " · " + (t.model || "modelo padrão") + (t.mode === "Direct" ? " · direto na pasta" : "") + " · #" + t.id,
        ...contextInfo(t.context),
      );
      bar.sub.hidden = false;
      pill.hidden = false;
      pill.className = "pill " + info.tone;
      pill.replaceChildren(info.tone === "err" ? icon("alert", 14) : h("span", { class: "pill-dot" }), info.label);
      document.title = sessionTitle(t.prompt, 40) + " · Argus";
    }
    const kind = t ? variant(t.state).kind : "";
    const active = isActive(kind);

    if (chat.id !== id || (!chat.loaded && !chat.error)) {
      total.textContent = "";
      chips.replaceChildren();
      painted = "";
      log.replaceChildren(loadingBox("Carregando a conversa…"));
      paintComposer();
      return;
    }
    if (chat.error && !chat.loaded) {
      painted = "";
      log.replaceChildren(errorBox(chat.error, () => loadChat(id)));
      paintComposer();
      return;
    }

    const totals = usageTotals(chat.events, S.pricing);
    total.textContent = totals.turns
      ? formatTokens(totals.tokens) + " tokens" + (totals.cost != null ? " · " + formatUsd(totals.cost) + " equivalente em API" : "")
      : "Sem consumo registrado";
    const subs = subagentTree(chat.events);
    const servers = serversOf(id).length;
    const run = S.aw.runs[id];
    const chipNodes = [];
    if (subs.length) chipNodes.push(h("a", { class: "chip", href: "#/s/" + id + "/sub", "data-chip": "subs" }, icon("bot", 16), plural(subs.length, "subagente", "subagentes")));
    if (servers) chipNodes.push(h("a", { class: "chip", href: "#/servidores" }, icon("server", 16), plural(servers, "servidor", "servidores")));
    if (run) chipNodes.push(h("a", { class: "chip " + awTone(run), href: "#/autowork" }, icon("loop", 16), awLabel(run)));
    chips.replaceChildren(...chipNodes);

    // A transcrição só é refeita quando algo dela mudou (não a cada consulta periódica).
    const sig = [chat.events.length, JSON.stringify(t ? t.state : null), !!S.pricing, document.documentElement.dataset.theme].join("|");
    if (sig === painted) {
      paintComposer();
      return;
    }
    painted = sig;
    const nodes = transcriptNodes(id, null, active, true);
    if (t && !chat.events.some((e) => e && typeof e === "object" && "User" in e) && t.prompt) nodes.unshift(h("div", { class: "turn" }, userMessage(t.prompt)));
    if (kind === "Failed") {
      nodes.push(h("div", { class: "fail", role: "alert" }, icon("alert", 18), h("span", null, h("b", { text: "Erro: a sessão falhou." }), h("span", { class: "fail-text", text: " " + String(variant(t.state).data || "") }))));
    }
    if (kind === "Cancelled") nodes.push(h("div", { class: "line-note" }, "Turno cancelado."));
    if (active) {
      const what = kind === "Queued" ? "na fila…" : liveActivity(chatEntries(chat.events, null));
      nodes.push(h("div", { class: "working", role: "status" }, h("span", { class: "spinner small" }), h("span", { class: "working-text", text: what })));
    }
    if (!nodes.length) nodes.push(emptyBox("chat", "Nenhuma mensagem ainda", "O histórico desta sessão está vazio."));
    const stuck = paintLog(view, log, nodes, first);
    if (!stuck && lastCount >= 0 && chat.events.length > lastCount) jump.hidden = false;
    if (stuck) jump.hidden = true;
    lastCount = chat.events.length;
    first = false;
    if (document.visibilityState === "visible" && S.unread[id]) markUnread(id, false);
    paintComposer();
  }

  markUnread(id, false);
  if (chat.id !== id || !chat.loaded) loadChat(id);
  grow();
  return {
    update,
    unmount: () => {
      view.removeEventListener("scroll", onScroll);
      document.title = "Argus";
    },
  };
};

/* ============================================================ telas: subagentes (somente leitura) */

VIEWS.subs = (r, top, view) => {
  const id = r.task;
  const bar = topBar(top, { back: () => back("s/" + id), title: "Subagentes" });
  bar.right.append(h("span", { class: "tag" }, icon("lock", 13), "somente leitura"));
  const box = h("div", { class: "page" });
  view.append(box);
  function update() {
    const t = taskById(id);
    bar.sub.textContent = t ? sessionTitle(t.prompt, 60) : "Sessão #" + id;
    bar.sub.hidden = false;
    if (chat.id !== id || (!chat.loaded && !chat.error)) {
      box.replaceChildren(loadingBox("Carregando os subagentes…"));
      return;
    }
    if (chat.error && !chat.loaded) {
      box.replaceChildren(errorBox(chat.error, () => loadChat(id)));
      return;
    }
    const active = t ? isActive(variant(t.state).kind) : false;
    const tree = subagentTree(chat.events);
    if (!tree.length) {
      box.replaceChildren(emptyBox("bot", "Nenhum subagente", "Esta sessão não delegou trabalho a subagentes."));
      return;
    }
    const running = tree.filter((n) => !n.done && active).length;
    box.replaceChildren(
      h("p", { class: "lead", text: plural(tree.length, "subagente", "subagentes") + (running ? " · " + running + " rodando" : "") + ". Toque em um para ver a conversa dele." }),
      h(
        "div",
        { class: "card list" },
        tree.map((n) => {
          const st = subagentStatus(n, active);
          const row = h(
            "a",
            { class: "trow", href: "#/s/" + id + "/sub/" + encodeURIComponent(n.id), "data-sub": n.id },
            statusMark(st.tone, st.label),
            h(
              "span",
              { class: "srow-main" },
              h("span", { class: "srow-title", text: n.description || n.subagent_type || n.id }),
              h("span", { class: "srow-line" }, h("span", { class: "srow-meta" }, h("span", { class: "st " + st.tone, text: st.label }), n.subagent_type ? h("span", { class: "sep", text: "·" }) : null, n.subagent_type || "")),
            ),
            icon("chevronRight", 18, "srow-chev"),
          );
          row.style.paddingLeft = 14 + Math.min(n.depth, 4) * 18 + "px";
          return row;
        }),
      ),
    );
  }
  if (chat.id !== id || !chat.loaded) loadChat(id);
  return { update };
};

VIEWS.subchat = (r, top, view, foot) => {
  const id = r.task;
  const subId = r.sub;
  const bar = topBar(top, { back: () => back("s/" + id + "/sub"), title: "Subagente" });
  const pill = h("span", { class: "pill" });
  bar.right.append(pill);
  const log = h("div", { class: "log" });
  view.className = "chat";
  view.append(log);
  // Sem caixa de resposta: a conversa de um subagente é só para leitura.
  foot.append(h("div", { class: "readonly-bar", "data-readonly": "1" }, icon("lock", 16), h("span", { text: "Somente leitura: não dá para responder a um subagente." })));
  let first = true;
  function update() {
    if (chat.id !== id || (!chat.loaded && !chat.error)) {
      log.replaceChildren(loadingBox("Carregando a conversa do subagente…"));
      return;
    }
    if (chat.error && !chat.loaded) {
      log.replaceChildren(errorBox(chat.error, () => loadChat(id)));
      return;
    }
    const t = taskById(id);
    const active = t ? isActive(variant(t.state).kind) : false;
    const node = subagentTree(chat.events).find((n) => n.id === subId);
    if (!node) {
      pill.hidden = true;
      log.replaceChildren(emptyBox("bot", "Subagente não encontrado", "Ele não aparece no histórico desta sessão."));
      return;
    }
    const st = subagentStatus(node, active);
    bar.h1.textContent = node.description || node.subagent_type || "Subagente";
    bar.sub.textContent = (node.subagent_type ? node.subagent_type + " · " : "") + "somente leitura";
    bar.sub.hidden = false;
    pill.hidden = false;
    pill.className = "pill " + st.tone;
    pill.replaceChildren(st.tone === "err" ? icon("alert", 14) : h("span", { class: "pill-dot" }), st.label);
    const nodes = [h("div", { class: "block" }, h("div", { class: "block-label", text: "Prompt recebido" }), h("div", { class: "block-box", text: node.prompt || "(vazio)" }))];
    const inner = transcriptNodes(id, subId, active, false);
    if (inner.length) nodes.push(h("div", { class: "block-label", text: "O que o subagente fez" }), ...inner);
    if (node.done) {
      nodes.push(
        h(
          "div",
          { class: "block" },
          h("div", { class: "block-label" + (node.ok === false ? " err" : "") }, node.ok === false ? icon("alert", 14) : null, node.ok === false ? "Erro devolvido a quem chamou" : "Resultado devolvido a quem chamou"),
          h("div", { class: "block-box md" + (node.ok === false ? " err" : ""), html: md(node.result || "(vazio)") }),
        ),
      );
    } else if (active) nodes.push(h("div", { class: "working", role: "status" }, h("span", { class: "spinner small" }), "trabalhando…"));
    else nodes.push(h("div", { class: "line-note", text: "Interrompido antes de devolver um resultado." }));
    paintLog(view, log, nodes, false);
    if (first) view.scrollTop = 0;
    first = false;
  }
  if (chat.id !== id || !chat.loaded) loadChat(id);
  return { update };
};

/* ============================================================ tela: nova conversa */

VIEWS.new = (r, top, view) => {
  topBar(top, { back: () => back(""), title: "Nova conversa" });
  const select = h("select", { id: "nf-project", class: "field", required: true });
  const provBox = h("div", { class: "seg", role: "radiogroup", "aria-label": "Provedor" });
  const prompt = h("textarea", { id: "nf-prompt", class: "field", rows: "6", placeholder: "Descreva o que o agente deve fazer…", required: true });
  prompt.value = loadKey("argus.rascunho.nova", "");
  const error = h("div", { class: "note err", role: "alert", hidden: true });
  const submit = h("button", { type: "submit", class: "btn primary wide" }, icon("send", 20), "Enviar");
  let provider = loadKey("argus.provedor", "");
  let sending = false;
  let wanted = r.project || loadKey("argus.projeto", "");
  let filled = "";
  // A conversa ainda não existe: os anexos vão para a pasta do rascunho.
  const draft = draftFor("nova");
  const atts = attachStrip(() => ({ draft }), () => paintSubmit());
  const attach = h("button", { type: "button", class: "btn wide" }, icon("image", 20), "Anexar da galeria");
  attach.addEventListener("click", () => atts.file.click());

  const form = h(
    "form",
    { class: "page form" },
    h("label", { class: "flabel", for: "nf-project", text: "Projeto" }),
    select,
    h("div", { class: "flabel", text: "Provedor" }),
    provBox,
    h("label", { class: "flabel", for: "nf-prompt", text: "Pedido" }),
    prompt,
    atts.strip,
    attach,
    atts.file,
    error,
    submit,
    h("p", { class: "hint", text: "O agente roda no computador, numa cópia isolada do projeto. Enviar inicia um turno de verdade, que consome a sua cota." }),
  );
  view.append(form);

  const paintSubmit = () => {
    submit.disabled = sending || atts.busy || !prompt.value.trim() || !select.value || !provider;
    submit.lastChild.textContent = sending ? "Enviando…" : atts.busy ? "Anexando…" : "Enviar";
  };
  prompt.addEventListener("input", () => {
    saveKey("argus.rascunho.nova", prompt.value);
    paintSubmit();
  });
  select.addEventListener("change", () => {
    wanted = select.value;
    paintSubmit();
  });

  form.addEventListener("submit", async (e) => {
    e.preventDefault();
    if (sending || !prompt.value.trim()) return;
    sending = true;
    error.hidden = true;
    paintSubmit();
    try {
      const res = await post("/api/tasks", { project: select.value, provider, prompt: prompt.value.trim() + atts.block() });
      saveKey("argus.projeto", select.value);
      saveKey("argus.provedor", provider);
      saveKey("argus.rascunho.nova", "");
      // Rascunho usado: o próximo pedido ganha uma pasta de anexos própria.
      saveKey("argus.anexos.nova", "");
      atts.clear();
      await loadTasks();
      // Troca a tela do formulário pela conversa criada (voltar cai na lista).
      navReplace = true;
      location.replace("#/s/" + res.id);
    } catch (err) {
      error.replaceChildren(icon("alert", 16), "Não foi possível criar a conversa: " + err.message);
      error.hidden = false;
    } finally {
      sending = false;
      paintSubmit();
    }
  });

  function update() {
    const projects = S.projects;
    const sig = projects.map((p) => p.path).join("|");
    if (sig !== filled) {
      filled = sig;
      select.replaceChildren(
        ...(projects.length ? [] : [h("option", { value: "", text: S.tasks == null ? "Carregando…" : "Nenhum projeto aberto no computador" })]),
        ...projects.map((p) => h("option", { value: p.path, text: p.name })),
      );
      const match = projects.find((p) => pathKey(p.path) === pathKey(wanted));
      if (match) select.value = match.path;
    }
    select.disabled = !projects.length;
    const provs = S.providers;
    if (provs.length && !provs.some((p) => p.id === provider)) provider = provs[0].id;
    provBox.replaceChildren(
      ...(provs.length
        ? provs.map((p) =>
            h("button", { type: "button", class: "seg-btn" + (p.id === provider ? " on" : ""), role: "radio", "aria-checked": String(p.id === provider), "data-provider": p.id, onclick: () => {
              provider = p.id;
              update();
            }, text: p.name }),
          )
        : [h("span", { class: "hint", text: "Carregando os provedores…" })]),
    );
    paintSubmit();
  }
  return { update };
};

/* ============================================================ tela: auto-work */

VIEWS.autowork = (_r, top, view) => {
  topBar(top, { title: "Auto-work" });
  const box = h("div", { class: "page" });
  view.append(box);
  const busy = new Set();

  async function toggle(project, enabled) {
    const key = pathKey(project);
    const before = S.aw.settings[key];
    S.aw.settings[key] = Object.assign({}, before || {}, { enabled });
    busy.add(key);
    update();
    try {
      const res = await post("/api/autowork", { project, enabled });
      S.aw.settings[key] = res.settings;
      toast("Auto-work " + (enabled ? "ligado" : "desligado") + " em " + projectName(project) + ".", "ok");
    } catch (e) {
      S.aw.settings[key] = before;
      toast("Não foi possível mudar o auto-work: " + e.message, "err");
    } finally {
      busy.delete(key);
      update();
    }
  }
  async function action(run, name) {
    if (name === "stop") {
      const ok = await confirmSheet({ title: "Parar o auto-work?", text: "A execução da sessão #" + run.task + " é encerrada. O turno em andamento é cancelado.", ok: "Parar", danger: true });
      if (!ok) return;
    }
    try {
      await post("/api/tasks/" + run.task + "/autowork/" + name);
      toast(name === "pause" ? "Auto-work pausado." : name === "resume" ? "Auto-work retomado." : "Auto-work parado.", "ok");
      loadAutowork();
    } catch (e) {
      toast("Não foi possível: " + e.message, "err");
    }
  }

  function runCard(run) {
    const st = variant(run.status);
    const tone = awTone(run);
    const label = st.kind === "Running" ? "rodando" : st.kind === "Paused" ? "pausado" : st.kind === "Completed" ? "concluído" : "parado";
    const t = taskById(run.task);
    const pct = run.total > 0 ? Math.round((run.done / run.total) * 100) : 0;
    const bar = h("div", { class: "progress", role: "progressbar", "aria-valuemin": "0", "aria-valuemax": String(run.total), "aria-valuenow": String(run.done), "aria-label": "Itens concluídos" }, h("div", { class: "progress-fill " + tone }));
    bar.firstChild.style.width = pct + "%";
    const live = st.kind === "Running" || st.kind === "Paused";
    return h(
      "div",
      { class: "runcard", "data-run": run.task },
      h("a", { class: "run-head", href: "#/s/" + run.task }, h("span", { class: "run-title", text: t ? sessionTitle(t.prompt, 60) : "Sessão #" + run.task }), h("span", { class: "pill " + tone }, h("span", { class: "pill-dot" }), label)),
      bar,
      h("div", { class: "run-nums", text: run.done + " de " + run.total + " itens · " + plural(run.blocked, "bloqueado", "bloqueados") + " · iteração " + run.iteration + "/" + run.max_iterations }),
      run.current_item ? h("div", { class: "run-item" }, h("span", { class: "block-label", text: "Item atual" }), h("span", { text: run.current_item })) : null,
      st.kind === "Stopped" && st.data ? h("div", { class: "run-item" }, h("span", { class: "block-label", text: "Motivo da parada" }), h("span", { text: String(st.data) })) : null,
      h("div", { class: "usage", text: formatTokens(run.tokens) + " tokens · " + formatUsd(run.cost_usd) + " equivalente em API" }),
      live
        ? h(
            "div",
            { class: "run-actions" },
            st.kind === "Running" ? h("button", { type: "button", class: "btn", "data-act": "pause", onclick: () => action(run, "pause"), text: "Pausar" }) : h("button", { type: "button", class: "btn primary", "data-act": "resume", onclick: () => action(run, "resume"), text: "Retomar" }),
            h("button", { type: "button", class: "btn danger", "data-act": "stop", onclick: () => action(run, "stop"), text: "Parar" }),
          )
        : null,
    );
  }

  function update() {
    if (!S.aw.loaded) {
      box.replaceChildren(loadingBox("Carregando o auto-work…"));
      return;
    }
    if (!S.aw.available) {
      box.replaceChildren(emptyBox("loop", "Auto-work indisponível", "O Argus do computador não expôs o auto-work nesta sessão."));
      return;
    }
    if (S.aw.error && !Object.keys(S.aw.settings).length && !S.projects.length) {
      box.replaceChildren(errorBox(S.aw.error, loadAutowork));
      return;
    }
    const projects = S.projects.map((p) => ({ name: p.name, path: p.path }));
    for (const run of Object.values(S.aw.runs)) {
      if (!projects.some((p) => pathKey(p.path) === pathKey(run.project))) projects.push({ name: baseName(run.project), path: run.project });
    }
    if (!projects.length) {
      box.replaceChildren(emptyBox("loop", "Nenhum projeto", "Abra uma pasta raiz no Argus do computador para ligar o auto-work por projeto."));
      return;
    }
    const nodes = [h("p", { class: "lead", text: "Com o auto-work ligado, o agente segue sozinho pelas pendências do projeto (PENDENCIAS.md, TODO e FIXME, build e testes) até os limites configurados no computador." })];
    if (S.aw.error) nodes.push(h("div", { class: "note err", role: "alert" }, icon("alert", 16), "Dados desatualizados: " + S.aw.error));
    for (const p of projects) {
      const key = pathKey(p.path);
      const s = S.aw.settings[key] || {};
      const on = !!s.enabled;
      const runs = Object.values(S.aw.runs).filter((x) => pathKey(x.project) === key).sort((a, b) => b.task - a.task);
      const limits = [];
      if (s.max_iterations) limits.push("até " + s.max_iterations + " iterações");
      if (s.max_tokens) limits.push(formatTokens(s.max_tokens) + " tokens");
      if (s.max_cost_usd) limits.push(formatUsd(s.max_cost_usd) + " equivalente em API");
      const sw = h(
        "button",
        { type: "button", class: "switch" + (on ? " on" : ""), role: "switch", "aria-checked": String(on), "aria-label": "Auto-work em " + p.name, "data-project": p.name, disabled: busy.has(key), onclick: () => toggle(p.path, !on) },
        h("span", { class: "switch-track" }, h("span", { class: "switch-thumb" })),
        h("span", { class: "switch-text", text: on ? "Ligado" : "Desligado" }),
      );
      nodes.push(
        h(
          "section",
          { class: "card" },
          h("div", { class: "card-head" }, h("div", { class: "card-title" }, h("b", { text: p.name }), h("span", { class: "card-sub", text: limits.length ? "Limites: " + limits.join(" · ") : "Limites padrão" })), sw),
          runs.length ? runs.map(runCard) : h("div", { class: "card-empty", text: on ? "Nenhuma execução em andamento. O auto-work começa quando uma sessão deste projeto termina um turno." : "Nenhuma execução." }),
        ),
      );
    }
    box.replaceChildren(...nodes);
  }
  loadAutowork();
  return { update };
};

/* ============================================================ tela: servidores */

VIEWS.servers = (_r, top, view) => {
  const bar = topBar(top, { title: "Servidores" });
  bar.right.append(h("button", { type: "button", class: "ibtn", "aria-label": "Atualizar", title: "Atualizar", onclick: () => loadServers() }, icon("refresh", 22)));
  const box = h("div", { class: "page" });
  view.append(box);
  const timer = setInterval(loadServers, 4000);

  async function kill(s) {
    const ok = await confirmSheet({
      title: "Derrubar o servidor da porta " + s.port + "?",
      text: "O processo " + s.name + " (pid " + s.pid + ") e os filhos dele são encerrados no computador.",
      detail: s.cmdline || null,
      ok: "Derrubar",
      danger: true,
    });
    if (!ok) return;
    try {
      S.servers = (await post("/api/servers/" + s.pid + "/kill")) || [];
      toast("Servidor da porta " + s.port + " derrubado.", "ok");
    } catch (e) {
      toast("Não foi possível derrubar: " + e.message, "err");
      loadServers();
    }
    update();
  }

  function update() {
    if (!S.serversLoaded) {
      box.replaceChildren(loadingBox("Procurando servidores…"));
      return;
    }
    if (S.serversError && !S.servers.length) {
      box.replaceChildren(errorBox(S.serversError, loadServers));
      return;
    }
    if (!S.servers.length) {
      box.replaceChildren(emptyBox("server", "Nenhum servidor rodando", "Quando uma sessão ou um terminal do Argus abrir uma porta no computador, ela aparece aqui."));
      return;
    }
    const groups = new Map();
    for (const s of S.servers) {
      const key = s.owner.kind + ":" + s.owner.id;
      if (!groups.has(key)) groups.set(key, { owner: s.owner, list: [] });
      groups.get(key).list.push(s);
    }
    const nodes = [];
    if (S.serversError) nodes.push(h("div", { class: "note err", role: "alert" }, icon("alert", 16), "Lista desatualizada: " + S.serversError));
    for (const g of groups.values()) {
      const t = g.owner.kind === "Session" ? taskById(g.owner.id) : null;
      const title = g.owner.kind === "Session" ? "Sessão #" + g.owner.id : "Terminal " + g.owner.id;
      const head =
        g.owner.kind === "Session"
          ? h("a", { class: "card-head link", href: "#/s/" + g.owner.id }, h("div", { class: "card-title" }, h("b", { text: title }), h("span", { class: "card-sub", text: t ? sessionTitle(t.prompt, 70) : "" })), icon("chevronRight", 18, "srow-chev"))
          : h("div", { class: "card-head" }, h("div", { class: "card-title" }, h("b", { text: title }), h("span", { class: "card-sub", text: "Terminal da IDE" })));
      nodes.push(
        h(
          "section",
          { class: "card" },
          head,
          g.list.map((s) =>
            h(
              "div",
              { class: "srv", "data-pid": s.pid },
              h("div", { class: "srv-top" }, h("span", { class: "srv-port", text: ":" + s.port }), h("span", { class: "srv-name", text: s.name + " · pid " + s.pid })),
              s.cmdline ? h("div", { class: "srv-cmd", text: s.cmdline }) : null,
              h(
                "div",
                { class: "run-actions" },
                h("a", { class: "btn", href: "http://" + location.hostname + ":" + s.port + "/", target: "_blank", rel: "noopener noreferrer", "data-act": "open" }, icon("link", 18), "Abrir"),
                h("button", { type: "button", class: "btn danger", "data-act": "kill", onclick: () => kill(s) }, icon("close", 18), "Derrubar"),
              ),
            ),
          ),
        ),
      );
    }
    nodes.push(h("p", { class: "hint", text: "“Abrir” usa o endereço do computador na rede. Um servidor que só escuta em localhost não abre pelo celular." }));
    box.replaceChildren(...nodes);
  }
  loadServers();
  return { update, unmount: () => clearInterval(timer) };
};

/* ============================================================ tela: ajustes */

VIEWS.settings = (_r, top, view) => {
  topBar(top, { title: "Ajustes" });
  const box = h("div", { class: "page" });
  view.append(box);
  function update() {
    const theme = document.documentElement.dataset.theme;
    const connText = S.conn === "on" ? "Conectado ao computador (ao vivo)" : S.conn === "off" ? "Sem conexão. Tentando reconectar…" : "Conectando…";
    box.replaceChildren(
      h("h2", { class: "sect", text: "Tema" }),
      h(
        "div",
        { class: "themes", role: "radiogroup", "aria-label": "Tema" },
        THEMES.map((t) =>
          h(
            "button",
            {
              type: "button",
              class: "theme" + (t.id === theme ? " on" : ""),
              role: "radio",
              "aria-checked": String(t.id === theme),
              "data-theme": t.id,
              "data-pick": t.id,
              onclick: () => {
                saveKey(THEME_KEY, applyTheme(t.id));
                const logo = document.querySelector(".logo");
                if (logo) logo.src = "/icon.svg?tema=" + t.id;
                update();
              },
            },
            h("span", { class: "theme-prev" }, h("span", { class: "theme-surface" }, h("span", { class: "theme-line" }), h("span", { class: "theme-line short" })), h("span", { class: "theme-accent" })),
            h("span", { class: "theme-text" }, h("b", { text: t.label }), h("span", { text: t.hint })),
            h("span", { class: "theme-check" }, t.id === theme ? icon("check", 18) : null),
          ),
        ),
      ),
      h("p", { class: "hint", text: "O tema fica guardado neste aparelho. As cores de estado (rodando, concluída, erro) são as mesmas nos três." }),
      h("h2", { class: "sect", text: "Conexão" }),
      h(
        "div",
        { class: "card pad" },
        h("div", { class: "kv" }, h("span", { class: "live " + S.conn }, h("span", { class: "live-dot" })), h("span", { text: connText })),
        h("div", { class: "kv dim", text: "Computador: " + location.host }),
        h("button", { type: "button", class: "btn", onclick: () => {
          reconnectNow();
          refreshAll();
          toast("Atualizando…");
        } }, icon("refresh", 18), "Atualizar agora"),
      ),
      h("h2", { class: "sect", text: "Acesso" }),
      h(
        "div",
        { class: "card pad" },
        h("p", { class: "kv", text: "Este aparelho entra pelo QR code da aba Remoto. Para revogar o acesso de todos os aparelhos, use “Gerar novo token” no computador." }),
        h("p", { class: "kv dim", text: "A conexão pela rede local não é criptografada: use só em redes de confiança, ou por VPN." }),
      ),
      ...(loadKey("argus.app", "") === "1"
        ? [
            h("h2", { class: "sect", text: "Servidor" }),
            h(
              "div",
              { class: "card pad" },
              h("p", { class: "kv", text: "Este é o aplicativo do Argus. Para apontar para outro computador, volte à tela de pareamento." }),
              h("button", { type: "button", class: "btn", onclick: () => history.back() }, icon("back", 18), "Trocar de servidor"),
            ),
          ]
        : [
            h("h2", { class: "sect", text: "Tela inicial" }),
            h("div", { class: "card pad" }, h("p", { class: "kv", text: "No menu do navegador, toque em “Adicionar à tela inicial” para abrir o Argus como um aplicativo." })),
          ]),
    );
  }
  return { update };
};

/* ============================================================ partida */

/** A caixa de composição acompanha a área visível: nunca fica atrás do teclado. */
function fitViewport() {
  const vv = window.visualViewport;
  const root = document.documentElement;
  if (vv) {
    root.style.setProperty("--app-h", Math.round(vv.height) + "px");
    root.style.setProperty("--app-top", Math.round(vv.offsetTop) + "px");
  } else {
    root.style.setProperty("--app-h", window.innerHeight + "px");
  }
}

function start() {
  fitViewport();
  if (window.visualViewport) {
    window.visualViewport.addEventListener("resize", fitViewport);
    window.visualViewport.addEventListener("scroll", fitViewport);
  }
  window.addEventListener("resize", fitViewport);
  window.addEventListener("hashchange", () => {
    trackNav();
    renderRoute();
  });
  window.addEventListener("online", reconnectNow);
  document.addEventListener("visibilitychange", () => {
    if (document.visibilityState !== "visible" || gated) return;
    // O sistema suspende a conexão com a tela apagada: ao voltar, reconecta e recarrega.
    if (!ws || ws.readyState > 1) reconnectNow();
    refreshAll();
  });

  // Pareado pelo aplicativo do celular (Tauri): a tela de pareamento dele fica
  // no histórico, então os Ajustes ganham "Trocar de servidor".
  if (/[?&]app=1/.test(location.search)) saveKey("argus.app", "1");
  if (/[?&]pareado=1/.test(location.search)) {
    try {
      history.replaceState(null, "", "/" + location.hash);
    } catch (_) {
      /* só estética */
    }
    setTimeout(() => toast("Aparelho pareado.", "ok"), 300);
  }
  renderRoute();
  loadTasks();
  loadProjects();
  loadStatic();
  loadAutowork();
  loadServers();
  connect();
  setInterval(checkBeat, 4000);
  // Os selos de servidores da lista dependem de uma consulta periódica.
  setInterval(() => {
    if (!gated && document.visibilityState === "visible" && current && current.name !== "servers") loadServers();
  }, 15000);
}

if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", start);
else start();
