/**
 * Comandos de barra da caixa de mensagem. Duas fontes:
 *
 * - locais do Argus: executam uma ação da IDE e NÃO vão ao agente;
 * - da CLI (skills, plugins, /compact...): vão como texto do turno. A lista vem
 *   de `chatAgentCapabilities` (só o claude informa).
 */

export type LocalCommandName = "nova" | "parar" | "diff" | "aplicar" | "descartar" | "sessoes" | "modelo" | "ajuda";

export interface LocalCommand {
  name: LocalCommandName;
  aliases: string[];
  description: string;
  /** Formato dos argumentos, se houver. */
  args?: string;
  /** Só faz sentido com uma tarefa aberta. */
  needsTask?: boolean;
}

export const LOCAL_COMMANDS: LocalCommand[] = [
  { name: "nova", aliases: ["new", "clear"], description: "Começa uma tarefa nova", args: "[pedido]" },
  { name: "parar", aliases: ["stop", "interrupt"], description: "Interrompe o turno em andamento", needsTask: true },
  { name: "diff", aliases: [], description: "Mostra as alterações da tarefa", needsTask: true },
  { name: "aplicar", aliases: ["apply"], description: "Aplica as alterações no projeto", needsTask: true },
  { name: "descartar", aliases: ["discard"], description: "Descarta as alterações da tarefa", needsTask: true },
  { name: "sessoes", aliases: ["sessions", "sessões"], description: "Lista as tarefas para trocar de conversa" },
  { name: "modelo", aliases: ["model", "models", "modelos"], description: "Escolhe o modelo da conversa", args: "[modelo]" },
  { name: "ajuda", aliases: ["help", "?"], description: "Lista os comandos" },
];

/** Comandos da CLI que só existem no terminal interativo: não entram nas sugestões. */
const TERMINAL_ONLY = new Set([
  "login",
  "logout",
  "config",
  "vim",
  "terminal-setup",
  "exit",
  "quit",
  "theme",
  "ide",
  "resume",
  "doctor",
]);

/**
 * Reconhece "/comando args" pelos nomes e apelidos locais; o resto não é comando
 * local. Só vale com a barra no começo: no meio de uma frase, a sugestão ajuda a
 * escrever, mas o texto vai inteiro ao agente.
 */
export function readLocalCommand(text: string): { command: LocalCommand; args: string } | null {
  const m = /^\/(\S+)\s*([\s\S]*)$/.exec(text.trim());
  if (!m) return null;
  const name = m[1].toLowerCase();
  const command = LOCAL_COMMANDS.find((c) => c.name === name || c.aliases.includes(name));
  return command ? { command, args: m[2].trim() } : null;
}

export interface SlashItem {
  /** Nome sem a barra (comandos de plugin vêm como "plugin:nome"). */
  name: string;
  description: string;
  local: boolean;
}

/** Junta locais e os da CLI, sem repetir nomes nem os que só existem no terminal. */
export function allSlashItems(cliCommands: string[]): SlashItem[] {
  const local: SlashItem[] = LOCAL_COMMANDS.map((c) => ({
    name: c.name,
    description: c.args ? `${c.description} · ${c.args}` : c.description,
    local: true,
  }));
  const taken = new Set(LOCAL_COMMANDS.flatMap((c) => [c.name, ...c.aliases]));
  const cli: SlashItem[] = [...new Set(cliCommands)]
    .filter((c) => c && !TERMINAL_ONLY.has(c) && !taken.has(c.toLowerCase()))
    .sort((a, b) => a.localeCompare(b))
    .map((c) => ({ name: c, description: c.includes(":") ? `da CLI · ${c.split(":")[0]}` : "da CLI", local: false }));
  return [...local, ...cli];
}

export interface SlashToken {
  /** Posição da "/" no texto. */
  start: number;
  /** Posição do cursor (fim do trecho digitado). */
  end: number;
  /** O que foi digitado depois da "/". */
  query: string;
}

/**
 * A "/trecho" que está sendo digitada na posição do cursor, em qualquer ponto da
 * mensagem (começo de linha, depois de um espaço ou de um parêntese). Como o "@",
 * não dispara dentro de um caminho ("src/views") nem de uma URL.
 */
export function slashCursor(text: string, cursor: number): SlashToken | null {
  const m = /(^|[\s(])\/([\w:.?-]*)$/.exec(text.slice(0, cursor));
  if (!m) return null;
  return { start: m.index + m[1].length, end: cursor, query: m[2].toLowerCase() };
}

/** Troca a "/trecho" pelo comando escolhido; devolve o texto novo e onde fica o cursor. */
export function replaceSlash(text: string, token: SlashToken, name: string): { text: string; cursor: number } {
  const inserted = `/${name} `;
  const rest = text.slice(token.end).replace(/^ /, "");
  return { text: text.slice(0, token.start) + inserted + rest, cursor: token.start + inserted.length };
}

/** Primeiro os que começam com o que foi digitado, depois os que contêm. */
export function filterSlash(items: SlashItem[], q: string, limit = 12): SlashItem[] {
  const short = (n: string) => n.split(":").pop()!.toLowerCase();
  const starts = items.filter((c) => c.name.toLowerCase().startsWith(q) || short(c.name).startsWith(q));
  const rest = items.filter((c) => !starts.includes(c) && c.name.toLowerCase().includes(q));
  return [...starts, ...rest].slice(0, limit);
}
