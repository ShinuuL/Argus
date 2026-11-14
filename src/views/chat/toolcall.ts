/**
 * Dois sintomas de agente travado, detectados por funções puras:
 *
 * - a chamada de ferramenta "escrita" como texto na resposta, que nunca executa
 *   (`toolCallAsText`), com o pedido fixo do botão "Executar de verdade";
 * - o loop de chamadas repetidas (`RepeatDetector`). Na interface ele só vale
 *   com a janela aberta; o lugar definitivo é o core (segunda leva, item 2.4).
 */

/** Mensagem do botão "Executar de verdade" quando o modelo só escreveu a chamada. */
export const RUN_FOR_REAL_PROMPT =
  "Você escreveu a chamada de ferramenta como texto na resposta, então nada foi executado. Use as ferramentas de verdade agora (sem reescrever o conteúdo no chat) e confirme o que foi feito.";

// Formatos de chamada de ferramenta que alguns modelos "imprimem" em vez de
// chamar: [tool_call tool=functions.write](...), <tool_call>, <function=...>,
// functions.edit({...}), {"name": "...", "arguments": ...}.
const CALL_AS_TEXT = [
  /\[tool_call\b/i,
  /<\/?tool_call>/i,
  /<function=\w+/i,
  /<\|tool_call/i,
  /\bfunctions\.\w+\s*\(\s*\{/,
  /\{\s*"name"\s*:\s*"[\w.-]+"\s*,\s*"(arguments|parameters|input)"\s*:/,
];

/**
 * A resposta tem uma chamada de ferramenta escrita como texto? Ignora blocos
 * de código (```), onde o agente pode estar só mostrando um exemplo.
 */
export function toolCallAsText(text: string): boolean {
  const withoutCode = text.replace(/```[\s\S]*?(```|$)/g, "");
  return CALL_AS_TEXT.some((r) => r.test(withoutCode));
}

/** Quantas repetições idênticas seguidas ainda são toleradas antes de parar. */
export const REPEATS_BEFORE_STOP = 2;
/** Quantas chamadas recentes entram na conta do ciclo. */
export const CYCLE_WINDOW = 8;
/** Quantas vezes a mesma chamada pode aparecer na janela antes de parar. */
export const TIMES_IN_CYCLE = 4;

/** JSON com chaves ordenadas: a mesma entrada sempre gera a mesma assinatura. */
function stable(v: unknown): string {
  if (v === null || v === undefined) return "null";
  if (typeof v !== "object") return JSON.stringify(v) ?? String(v);
  if (Array.isArray(v)) return "[" + v.map(stable).join(",") + "]";
  const o = v as Record<string, unknown>;
  return (
    "{" +
    Object.keys(o)
      .sort()
      .map((k) => JSON.stringify(k) + ":" + stable(o[k]))
      .join(",") +
    "}"
  );
}

/** Identidade de uma chamada: nome da ferramenta + entrada normalizada. */
export function signature(name: unknown, input: unknown): string {
  return `${String(name ?? "")}:${stable(input)}`;
}

export type RepeatReason = "seguidas" | "ciclo";

/** Texto do aviso mostrado quando o detector dispara. */
export function repeatMessage(tool: string, reason: RepeatReason): string {
  return reason === "seguidas"
    ? `Parei: o agente repetiu ${tool} ${REPEATS_BEFORE_STOP + 1} vezes seguidas com a mesma entrada.`
    : `Parei: o agente voltou ${TIMES_IN_CYCLE} vezes à mesma chamada (${tool}) nas últimas ${CYCLE_WINDOW} ações.`;
}

/**
 * Conta as chamadas de ferramenta de um turno e avisa quando há repetição:
 * a MESMA chamada várias vezes seguidas (A, A, A) ou um ciclo curto que volta
 * sempre a ela (A, B, A, B, A, B, A). Quem decide interromper é quem usa.
 */
export class RepeatDetector {
  private last: { signature: string; times: number } | null = null;
  private recent: string[] = [];

  constructor(
    private onRepeat: (tool: string, reason: RepeatReason) => void,
    private limit = REPEATS_BEFORE_STOP,
  ) {}

  /** Esquece o que viu (novo turno). */
  reset(): void {
    this.last = null;
    this.recent = [];
  }

  /** Conta uma chamada; dispara `onRepeat` quando passa do limite. */
  tool(name: unknown, input: unknown): void {
    const sig = signature(name, input);
    this.recent.push(sig);
    if (this.recent.length > CYCLE_WINDOW) this.recent.shift();

    if (this.last?.signature === sig) {
      this.last.times++;
      if (this.last.times >= this.limit) this.stop(name, "seguidas");
      return;
    }
    this.last = { signature: sig, times: 0 };
    if (this.recent.filter((x) => x === sig).length >= TIMES_IN_CYCLE) this.stop(name, "ciclo");
  }

  private stop(name: unknown, reason: RepeatReason) {
    this.reset();
    this.onRepeat(String(name ?? "ferramenta"), reason);
  }
}
