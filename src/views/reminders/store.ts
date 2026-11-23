/**
 * Lembretes do usuário: lista guardada no banco (`reminders_get/set`) e um
 * relógio único, ligado na raiz do app, que dispara o aviso `before` minutos
 * antes de `at`. Um lembrete perdido com o app fechado avisa ao abrir, se o
 * compromisso ainda não passou há mais de 1 hora.
 */
import { create } from "zustand";
import { api, errorMessage, type Reminder } from "@/api";
import { whenLabel } from "./parse";

interface RemindersState {
  list: Reminder[];
  loaded: boolean;
  /** Lembretes avisados e ainda não dispensados (o aviso na tela). */
  alerts: Reminder[];
  error: string | null;
  load(): Promise<void>;
  add(r: Omit<Reminder, "id" | "fired">): Promise<void>;
  remove(id: string): Promise<void>;
  dismiss(id: string): void;
  /** Adia o aviso: volta a avisar daqui a `min` minutos. */
  snooze(id: string, min: number): Promise<void>;
  tick(now?: number): void;
}

const LATE_LIMIT = 60 * 60_000;

export const useReminders = create<RemindersState>((set, get) => {
  const save = async (list: Reminder[]) => {
    set({ list });
    try {
      await api.remindersSet(list);
      set({ error: null });
    } catch (e) {
      set({ error: errorMessage(e) });
    }
  };
  return {
    list: [],
    loaded: false,
    alerts: [],
    error: null,
    async load() {
      try {
        set({ list: await api.remindersGet(), loaded: true });
      } catch (e) {
        set({ loaded: true, error: errorMessage(e) });
      }
    },
    async add(r) {
      const id = `${Date.now().toString(36)}${Math.random().toString(36).slice(2, 6)}`;
      const list = [...get().list, { ...r, id, fired: false }].sort((a, b) => a.at - a.before * 60_000 - (b.at - b.before * 60_000));
      await save(list);
      get().tick();
    },
    async remove(id) {
      set({ alerts: get().alerts.filter((a) => a.id !== id) });
      await save(get().list.filter((r) => r.id !== id));
    },
    dismiss(id) {
      set({ alerts: get().alerts.filter((a) => a.id !== id) });
    },
    async snooze(id, min) {
      const r = get().list.find((x) => x.id === id);
      set({ alerts: get().alerts.filter((a) => a.id !== id) });
      if (!r) return;
      // Avisa de novo em `min` minutos: o "antes" passa a ser a distância até lá.
      const again = Date.now() + min * 60_000;
      const before = Math.max(0, Math.round((r.at - again) / 60_000));
      await save(get().list.map((x) => (x.id === id ? { ...x, before, at: before === 0 ? again : x.at, fired: false } : x)));
    },
    tick(now = Date.now()) {
      const due = get().list.filter((r) => !r.fired && now >= r.at - r.before * 60_000 && now - r.at < LATE_LIMIT);
      const stale = get().list.filter((r) => !r.fired && now - r.at >= LATE_LIMIT);
      if (due.length === 0 && stale.length === 0) return;
      const ids = new Set([...due, ...stale].map((r) => r.id));
      set({ alerts: [...get().alerts, ...due] });
      void save(get().list.map((r) => (ids.has(r.id) ? { ...r, fired: true } : r)));
      if (due.length) {
        void attention();
        for (const r of due) void api.notifyUser(`Lembrete: ${r.text}`, whenLabel(r.at)).catch(() => undefined);
      }
    },
  };
});

/** Pisca a janela na barra de tarefas (o Windows pede atenção sem roubar o foco). */
async function attention() {
  try {
    const { getCurrentWindow, UserAttentionType } = await import("@tauri-apps/api/window");
    await getCurrentWindow().requestUserAttention(UserAttentionType.Critical);
  } catch {
    // Fora do Tauri (navegador de teste): o aviso na tela basta.
  }
}

let started = false;
/** Liga o relógio uma vez (chamado na raiz do app). */
export function startReminders() {
  if (started) return;
  started = true;
  void useReminders.getState().load().then(() => useReminders.getState().tick());
  setInterval(() => useReminders.getState().tick(), 15_000);
}
