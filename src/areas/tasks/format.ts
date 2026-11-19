/** Textos em pt-BR das tarefas agendadas (funções puras). */
import type { Recurrence, RunStatus } from "@/api";
import type { Tone } from "@/ui";

export const WEEKDAYS = ["dom", "seg", "ter", "qua", "qui", "sex", "sáb"];
const WEEKDAYS_LONG = ["domingo", "segunda", "terça", "quarta", "quinta", "sexta", "sábado"];

const two = (n: number) => String(n).padStart(2, "0");
const hhmm = (d: Date) => `${two(d.getHours())}:${two(d.getMinutes())}`;

function startOfDay(d: Date): number {
  return new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
}

/** "hoje às 09:00", "amanhã às 09:00", "seg., 23/11 às 18:30" ou com o ano, se for outro. */
export function formatWhen(secs: number, now: Date = new Date()): string {
  const d = new Date(secs * 1000);
  const days = Math.round((startOfDay(d) - startOfDay(now)) / 86_400_000);
  if (days === 0) return `hoje às ${hhmm(d)}`;
  if (days === 1) return `amanhã às ${hhmm(d)}`;
  if (days === -1) return `ontem às ${hhmm(d)}`;
  const date = `${two(d.getDate())}/${two(d.getMonth() + 1)}${d.getFullYear() !== now.getFullYear() ? `/${d.getFullYear()}` : ""}`;
  return `${WEEKDAYS[d.getDay()]}., ${date} às ${hhmm(d)}`;
}

/** Recorrência legível: "Todo dia às 09:00", "Seg, qua e sex às 18:30"… */
export function describeRecurrence(r: Recurrence): string {
  switch (r.kind) {
    case "Once":
      return `Uma vez, ${formatWhen(r.at)}`;
    case "Daily":
      return `Todo dia às ${r.time}`;
    case "Weekly": {
      const days = [...r.days].sort((a, b) => a - b);
      const key = days.join(",");
      if (key === "1,2,3,4,5") return `Dias úteis às ${r.time}`;
      if (key === "0,6") return `Fins de semana às ${r.time}`;
      if (days.length === 7) return `Todo dia às ${r.time}`;
      if (days.length === 1) return `Toda ${WEEKDAYS_LONG[days[0]]} às ${r.time}`.replace("Toda domingo", "Todo domingo").replace("Toda sábado", "Todo sábado");
      const names = days.map((d) => WEEKDAYS[d]);
      const list = `${names.slice(0, -1).join(", ")} e ${names[names.length - 1]}`;
      return `${list.charAt(0).toUpperCase()}${list.slice(1)} às ${r.time}`;
    }
    case "EveryHours":
      return r.hours === 1 ? "A cada hora" : `A cada ${r.hours} horas`;
    case "Cron":
      return `Cron: ${r.expr}`;
  }
}

/** Rótulo e tom do último resultado. */
export function runStatusLabel(s: RunStatus | null, late: boolean): { label: string; tone?: Tone; detail?: string } {
  const suffix = late ? " (atrasada)" : "";
  if (s == null) return { label: "Ainda não rodou" };
  if (typeof s === "object") return { label: `Falhou${suffix}`, tone: "bad", detail: s.Failed };
  switch (s) {
    case "Running":
      return { label: `Rodando${suffix}`, tone: "info" };
    case "Completed":
      return { label: `Concluída${suffix}`, tone: "ok" };
    case "Cancelled":
      return { label: `Cancelada${suffix}`, tone: "warn" };
    case "Reminded":
      return { label: `Lembrete mostrado${suffix}`, tone: "ok" };
  }
}

/** `datetime-local` (horário local) ↔ segundos. */
export function toLocalInput(secs: number): string {
  const d = new Date(secs * 1000);
  return `${d.getFullYear()}-${two(d.getMonth() + 1)}-${two(d.getDate())}T${hhmm(d)}`;
}

export function fromLocalInput(v: string): number | null {
  const t = new Date(v).getTime();
  return Number.isFinite(t) ? Math.floor(t / 1000) : null;
}
