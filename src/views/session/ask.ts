/**
 * Perguntas de múltipla escolha que a CLI faz no meio do turno.
 *
 * Elas chegam como uma chamada de ferramenta comum (`AskUserQuestion` no
 * Claude; outros nomes em outras CLIs), então sem isto virariam só mais uma
 * linha de ferramenta com o JSON cru. Aqui o formato é reconhecido e vira um
 * cartão com as opções clicáveis; a escolha volta como mensagem do usuário.
 *
 * O formato varia entre CLIs e entre versões, então o parse é tolerante:
 * qualquer coisa que não tenha pergunta e opções devolve null e cai no
 * caminho antigo (linha de ferramenta).
 */

/** Nomes de ferramenta que são pergunta ao usuário. */
const ASK_TOOLS = new Set(["askuserquestion", "ask_user_question", "ask_followup_question", "askquestion"]);

export function isAskTool(name: string): boolean {
  return ASK_TOOLS.has(name.toLowerCase().replace(/^mcp__[^_]+__/, ""));
}

/** Identificador de uma pergunta feita pelo protocolo ACP. */
export function acpAskRef(input: unknown): string | null {
  if (!input || typeof input !== "object" || Array.isArray(input)) return null;
  const value = (input as Record<string, unknown>).acp_request;
  return typeof value === "string" && value ? value : null;
}

export interface AskOption {
  label: string;
  description: string | null;
}

export interface AskQuestion {
  /** Título curto, quando a CLI manda um. */
  header: string | null;
  question: string;
  options: AskOption[];
  /** Aceita mais de uma opção. */
  multi: boolean;
}

function str(v: unknown): string | null {
  return typeof v === "string" && v.trim() ? v.trim() : null;
}

function obj(v: unknown): Record<string, unknown> | null {
  return v != null && typeof v === "object" && !Array.isArray(v) ? (v as Record<string, unknown>) : null;
}

/** Uma opção: texto puro ou objeto com rótulo e descrição. */
function parseOption(raw: unknown): AskOption | null {
  const direct = str(raw);
  if (direct) return { label: direct, description: null };
  const o = obj(raw);
  if (!o) return null;
  const label = str(o.label) ?? str(o.name) ?? str(o.title) ?? str(o.value) ?? str(o.option);
  if (!label) return null;
  return { label, description: str(o.description) ?? str(o.detail) ?? str(o.hint) };
}

function parseQuestion(raw: unknown): AskQuestion | null {
  const o = obj(raw);
  if (!o) return null;
  const question = str(o.question) ?? str(o.prompt) ?? str(o.text) ?? str(o.header);
  if (!question) return null;
  const list = o.options ?? o.choices ?? o.answers;
  const options = Array.isArray(list) ? list.map(parseOption).filter((x): x is AskOption => x != null) : [];
  if (options.length === 0) return null;
  const header = str(o.header);
  return {
    header: header && header !== question ? header : null,
    question,
    options,
    multi: o.multiSelect === true || o.multi_select === true || o.multiple === true,
  };
}

/**
 * Lê a entrada da ferramenta e devolve as perguntas, ou null se não der para
 * mostrar como cartão (formato desconhecido, sem opções...).
 */
/**
 * No `claude -p` a ferramenta de pergunta não existe: o Argus pede ao agente
 * (prompt de sistema) que escreva as perguntas com opções num bloco
 * ```argus-ask com JSON `{ "questions": [...] }`, no mesmo formato da ferramenta.
 * Devolve o texto sem o bloco e as perguntas lidas.
 *
 * O bloco sai do texto mesmo quando não dá para lê-lo (`quebrado`): antes ele
 * voltava inteiro pelo caminho do markdown, e uma vírgula faltando no JSON
 * virava mil caracteres de JSON cru na cara do usuário, sem pergunta nenhuma e
 * sem dizer que havia uma. Agora vira uma linha curta — o agente erra igual, mas
 * o erro fica legível e a conversa continua.
 */
export function splitAskBlock(text: string): { text: string; ask: AskQuestion[] | null; quebrado: boolean } {
  const m = /```argus-ask\s*\n([\s\S]*?)\n?```/.exec(text);
  if (!m) return { text, ask: null, quebrado: false };
  let ask: AskQuestion[] | null = null;
  try {
    ask = parseAsk(JSON.parse(m[1]));
  } catch {
    ask = null;
  }
  const sem = (text.slice(0, m.index) + text.slice(m.index + m[0].length)).trim();
  return { text: sem, ask, quebrado: ask == null };
}

export function parseAsk(input: unknown): AskQuestion[] | null {
  const o = obj(input);
  if (!o) return null;
  const raw = o.questions ?? o.question ?? o.items ?? input;
  const list = Array.isArray(raw) ? raw : [raw];
  const questions = list.map(parseQuestion).filter((q): q is AskQuestion => q != null);
  return questions.length > 0 ? questions : null;
}

/**
 * Texto da resposta enviada ao agente. Uma pergunta só com uma escolha vai
 * como a própria opção (é o que o agente espera ler); o resto vai rotulado,
 * para não ficar ambíguo qual resposta é de qual pergunta.
 */
export function answerText(questions: AskQuestion[], picked: string[][], extra: string): string {
  const parts: string[] = [];
  questions.forEach((q, i) => {
    const chosen = picked[i] ?? [];
    if (chosen.length === 0) return;
    parts.push(questions.length === 1 && chosen.length === 1 ? chosen[0] : `${q.header ?? q.question}: ${chosen.join(", ")}`);
  });
  const note = extra.trim();
  if (note) parts.push(parts.length ? `Observação: ${note}` : note);
  return parts.join("\n");
}
