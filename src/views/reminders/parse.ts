/**
 * Lê um lembrete escrito do jeito que se fala:
 *   "tenho algo a fazer 18h me lembre 30 minutos antes"
 *   "reunião amanhã 9:30, avisar 1 hora antes"
 * Devolve o texto, a hora do compromisso e quantos minutos antes avisar.
 */
export interface ParsedReminder {
  text: string;
  at: number;
  before: number;
}

const BEFORE = /(\d+)\s*(min(?:uto)?s?|h(?:ora)?s?)\s*antes/i;
// Sem `\b` perto de letras acentuadas: no JavaScript ele não as conta como letra.
const TIME = /(?:(?<=^|\s)(?:às|as|a)\s+)?\b([01]?\d|2[0-3])\s*(?:h|:)\s*([0-5]\d)?(?:\s*min)?\b/i;
const TOMORROW = /(?<=^|\s)amanh[ãa](?=$|[\s,.;])/i;
/** Palavras de pedido que não fazem parte do que fazer. */
const NOISE = /\b(me\s+)?(lembre|lembra|lembrar|avise|avisa|avisar)(-me)?\b|\bme\b(?=\s*$)/gi;

export function parseReminder(input: string, now = new Date()): ParsedReminder | null {
  let s = input.trim();
  const t = TIME.exec(s.replace(BEFORE, ""));
  if (!t) return null;
  const hour = Number(t[1]);
  const minute = t[2] ? Number(t[2]) : 0;

  let before = 0;
  const b = BEFORE.exec(s);
  if (b) before = /^h/i.test(b[2]) ? Number(b[1]) * 60 : Number(b[1]);

  const when = new Date(now);
  when.setSeconds(0, 0);
  when.setHours(hour, minute);
  if (TOMORROW.test(s) || when.getTime() <= now.getTime()) when.setDate(when.getDate() + 1);

  s = s.replace(BEFORE, " ").replace(TIME, " ").replace(TOMORROW, " ").replace(NOISE, " ");
  const text = s.replace(/\s*[,.;:-]\s*/g, " ").replace(/\s+/g, " ").trim().replace(/\s+(e|para|pra|a)$/i, "").trim();
  return { text: text || "Lembrete", at: when.getTime(), before };
}

/** "hoje às 18:00", "amanhã às 09:30" ou "20/11 às 08:00". */
export function whenLabel(at: number, now = new Date()): string {
  const d = new Date(at);
  const hm = d.toLocaleTimeString("pt-BR", { hour: "2-digit", minute: "2-digit" });
  const day = new Date(now);
  day.setHours(0, 0, 0, 0);
  const diff = Math.round((new Date(d).setHours(0, 0, 0, 0) - day.getTime()) / 86_400_000);
  if (diff === 0) return `hoje às ${hm}`;
  if (diff === 1) return `amanhã às ${hm}`;
  return `${d.toLocaleDateString("pt-BR", { day: "2-digit", month: "2-digit" })} às ${hm}`;
}

/** "30 min antes", "1 h antes", "1 h 30 min antes" ou "na hora". */
export function beforeLabel(min: number): string {
  if (min <= 0) return "na hora";
  const h = Math.floor(min / 60);
  const m = min % 60;
  return `${h ? `${h} h` : ""}${h && m ? " " : ""}${m ? `${m} min` : ""} antes`;
}
