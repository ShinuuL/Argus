/** Diff unificado do git em arquivos com linhas (número antigo e novo), para as vistas lado a lado e unificada. */

export type Kind = "ctx" | "add" | "del" | "hunk";

export interface Row {
  kind: Kind;
  text: string;
  old?: number;
  new?: number;
}

export interface FileDiff {
  path: string;
  add: number;
  del: number;
  binary: boolean;
  rows: Row[];
}

export function parseDiff(diff: string): FileDiff[] {
  const files: FileDiff[] = [];
  let cur: FileDiff | null = null;
  let o = 0;
  let n = 0;
  for (const raw of diff.split("\n")) {
    const line = raw.replace(/\r$/, "");
    const head = /^diff --git a\/(.+?) b\/(.+)$/.exec(line);
    if (head) {
      cur = { path: head[2], add: 0, del: 0, binary: false, rows: [] };
      files.push(cur);
      continue;
    }
    if (!cur) continue;
    const hunk = /^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@(.*)$/.exec(line);
    if (hunk) {
      o = Number(hunk[1]);
      n = Number(hunk[2]);
      cur.rows.push({ kind: "hunk", text: hunk[3].trim() });
      continue;
    }
    if (line.startsWith("Binary files")) {
      cur.binary = true;
      continue;
    }
    if (line.startsWith("+++") || line.startsWith("---") || line.startsWith("index ") || line.startsWith("\\")) continue;
    if (/^(new|deleted) file mode|^similarity|^rename |^old mode|^new mode/.test(line)) continue;
    if (line.startsWith("+")) {
      cur.add++;
      cur.rows.push({ kind: "add", text: line.slice(1), new: n++ });
    } else if (line.startsWith("-")) {
      cur.del++;
      cur.rows.push({ kind: "del", text: line.slice(1), old: o++ });
    } else if (line.startsWith(" ")) {
      cur.rows.push({ kind: "ctx", text: line.slice(1), old: o++, new: n++ });
    }
  }
  return files;
}

/** Pares esquerda/direita: contexto nos dois lados, blocos de remoção e adição alinhados lado a lado. */
export function sideBySide(rows: Row[]): [Row | null, Row | null][] {
  const out: [Row | null, Row | null][] = [];
  let i = 0;
  while (i < rows.length) {
    const r = rows[i];
    if (r.kind === "ctx" || r.kind === "hunk") {
      out.push([r, r]);
      i++;
      continue;
    }
    const dels: Row[] = [];
    const adds: Row[] = [];
    while (i < rows.length && rows[i].kind === "del") dels.push(rows[i++]);
    while (i < rows.length && rows[i].kind === "add") adds.push(rows[i++]);
    for (let k = 0; k < Math.max(dels.length, adds.length); k++) out.push([dels[k] ?? null, adds[k] ?? null]);
  }
  return out;
}
