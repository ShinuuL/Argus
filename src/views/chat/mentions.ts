/**
 * Menções com "@" na caixa de mensagem: @arquivo (caminho relativo ao diretório
 * de trabalho da tarefa) e @agente. Funções puras; a lista de arquivos vem do
 * backend (`chatProjectFiles`), que já devolve ranqueado.
 */

/** Pontua um caminho para a busca: nome que começa igual vale mais que trecho no meio do caminho. */
export function score(rel: string, q: string): number {
  const target = rel.toLowerCase();
  const name = target.slice(target.lastIndexOf("/") + 1);
  const query = q.toLowerCase().replace(/\\/g, "/");
  if (!query) return 1;
  if (name === query) return 1000;
  if (name.startsWith(query)) return 800 - name.length;
  if (name.includes(query)) return 600 - name.length;
  if (target.includes(query)) return 400 - target.length / 10;
  // letras na ordem, com buracos ("extts" acha "extension.ts")
  let i = 0;
  for (const c of name) if (c === query[i]) i++;
  if (i === query.length) return 200 - name.length;
  return 0;
}

/** Os melhores itens para a busca, em ordem. Rasos primeiro no empate. */
export function search(paths: string[], q: string, limit = 30): string[] {
  const found: { rel: string; points: number }[] = [];
  for (const rel of paths) {
    const points = score(rel, q);
    if (points > 0) found.push({ rel, points });
  }
  found.sort(
    (a, b) => b.points - a.points || a.rel.split("/").length - b.rel.split("/").length || a.rel.localeCompare(b.rel),
  );
  return found.slice(0, limit).map((a) => a.rel);
}

export interface AtToken {
  /** Posição do "@" no texto. */
  start: number;
  /** Posição do cursor (fim do trecho digitado). */
  end: number;
  /** O que foi digitado depois do "@". */
  query: string;
}

/** O "@trecho" que está sendo digitado na posição do cursor, se houver. */
export function atCursor(text: string, cursor: number): AtToken | null {
  const m = /(^|[\s(])@([^\s@]*)$/.exec(text.slice(0, cursor));
  if (!m) return null;
  const start = m.index + m[1].length;
  return { start, end: cursor, query: m[2] };
}

/** Troca o "@trecho" pelo item escolhido; devolve o texto novo e onde fica o cursor. */
export function replaceAt(text: string, token: AtToken, value: string): { text: string; cursor: number } {
  const inserted = `@${value} `;
  const rest = text.slice(token.end).replace(/^ /, "");
  return { text: text.slice(0, token.start) + inserted + rest, cursor: token.start + inserted.length };
}

/** Recusa caminhos que saem da pasta do projeto. */
export function isSafeRelative(rel: string): boolean {
  return !!rel && !/^([\\/]|[a-zA-Z]:)/.test(rel) && !rel.split(/[\\/]/).includes("..");
}
