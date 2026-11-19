/**
 * Área "Tarefas": pedidos agendados a um agente (ou lembretes) que disparam num
 * horário. Rodam só com o Argus aberto; o que venceu com ele fechado roda uma vez
 * ao abrir (marcado "atrasada"). Cada execução vira uma conversa normal.
 */
import { useCallback, useEffect, useState } from "react";
import {
  api,
  baseName,
  errorMessage,
  onSchedulesChanged,
  samePath,
  type ModelInfo,
  type ProviderInfo,
  type Recurrence,
  type Schedule,
  type WorkMode,
} from "@/api";
import { useApp, useWorkbench } from "@/store";
import { Select } from "@/shell/Select";
import { AreaHeader, AreaStatus, Btn, Chip, DataTable, Seg, type Column } from "@/ui";
import { describeRecurrence, formatWhen, fromLocalInput, runStatusLabel, toLocalInput, WEEKDAYS } from "./format";
import { onNewTaskRequest, takeNewTaskRequest } from "./intent";
import "./tasks.css";

type RecKind = Recurrence["kind"];

const REC_OPTIONS: { value: RecKind; label: string }[] = [
  { value: "Once", label: "Uma vez" },
  { value: "Daily", label: "Diária" },
  { value: "Weekly", label: "Dias da semana" },
  { value: "EveryHours", label: "A cada N horas" },
  { value: "Cron", label: "Cron" },
];

/** Rascunho do formulário: guarda os campos de todas as recorrências para trocar sem perder. */
interface Draft {
  s: Schedule;
  recKind: RecKind;
  onceAt: string;
  time: string;
  days: number[];
  hours: number;
  cron: string;
}

function blank(project: string | null, provider: string): Schedule {
  return {
    id: 0,
    name: "",
    kind: "Agent",
    project,
    provider,
    model: null,
    mode: null,
    prompt: "",
    recurrence: { kind: "Daily", time: "09:00" },
    enabled: true,
    created_at: 0,
    last_run: null,
    next_run: null,
    last_task: null,
    last_status: null,
    last_late: false,
  };
}

function toDraft(s: Schedule): Draft {
  const r = s.recurrence;
  const inAnHour = Math.floor(Date.now() / 1000 / 60) * 60 + 3600;
  return {
    s,
    recKind: r.kind,
    onceAt: toLocalInput(r.kind === "Once" ? r.at : inAnHour),
    time: r.kind === "Daily" || r.kind === "Weekly" ? r.time : "09:00",
    days: r.kind === "Weekly" ? r.days : [1, 2, 3, 4, 5],
    hours: r.kind === "EveryHours" ? r.hours : 4,
    cron: r.kind === "Cron" ? r.expr : "0 9 * * 1-5",
  };
}

function fromDraft(d: Draft): Schedule | string {
  let recurrence: Recurrence;
  switch (d.recKind) {
    case "Once": {
      const at = fromLocalInput(d.onceAt);
      if (at == null) return "Escolha a data e a hora.";
      recurrence = { kind: "Once", at };
      break;
    }
    case "Daily":
      recurrence = { kind: "Daily", time: d.time };
      break;
    case "Weekly":
      recurrence = { kind: "Weekly", days: d.days, time: d.time };
      break;
    case "EveryHours":
      recurrence = { kind: "EveryHours", hours: Math.round(d.hours) };
      break;
    case "Cron":
      recurrence = { kind: "Cron", expr: d.cron.trim() };
      break;
  }
  return { ...d.s, recurrence };
}

function useProviderList(): ProviderInfo[] {
  const [list, setList] = useState<ProviderInfo[]>([]);
  useEffect(() => {
    api.agentsProviders().then(setList, () => {});
  }, []);
  return list;
}

function Editor({ initial, providers, onClose, onSaved }: { initial: Schedule; providers: ProviderInfo[]; onClose(): void; onSaved(s: Schedule): void }) {
  const projects = useApp((s) => s.projects);
  const [d, setD] = useState<Draft>(() => toDraft(initial));
  const [models, setModels] = useState<ModelInfo[] | null>(null);
  const [modelNote, setModelNote] = useState<string | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const s = d.s;
  const set = (patch: Partial<Schedule>) => setD((x) => ({ ...x, s: { ...x.s, ...patch } }));
  const agent = s.kind === "Agent";

  useEffect(() => setModels(null), [s.provider]);
  const loadModels = () => {
    if (models || !s.provider) return;
    setModelNote("Carregando modelos…");
    api.agentsModels(s.provider).then(
      (l) => (setModels(l), setModelNote(null)),
      (e) => setModelNote(errorMessage(e)),
    );
  };

  const save = async () => {
    const out = fromDraft(d);
    if (typeof out === "string") return setErr(out);
    setBusy(true);
    setErr(null);
    try {
      onSaved(await api.schedulesSave(out));
    } catch (e) {
      setErr(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  const modelOptions = [
    { value: "", label: "Padrão da CLI" },
    ...(models ?? []).map((m) => ({ value: m.value, label: m.label, hint: m.hint || undefined })),
  ];
  if (s.model && !modelOptions.some((o) => o.value === s.model)) modelOptions.push({ value: s.model, label: s.model });

  return (
    <aside className="tka-panel" aria-label={s.id ? "Editar tarefa" : "Nova tarefa"}>
      <form
        className="tka-form"
        onSubmit={(e) => {
          e.preventDefault();
          void save();
        }}
      >
        <p className="tka-h">{s.id ? "Editar tarefa" : "Nova tarefa"}</p>
        <label className="tka-field">
          <span>Nome</span>
          <input value={s.name} onChange={(e) => set({ name: e.target.value })} placeholder="Revisar dependências" autoFocus />
        </label>
        <div className="tka-field">
          <span>O que fazer</span>
          <Seg<"Agent" | "Reminder">
            label="O que fazer"
            value={s.kind}
            onChange={(kind) => set({ kind })}
            options={[
              { value: "Agent", label: "Pedir a um agente" },
              { value: "Reminder", label: "Só lembrar" },
            ]}
          />
        </div>
        {agent && (
          <>
            <label className="tka-field">
              <span>Projeto</span>
              <Select
                ariaLabel="Projeto"
                value={s.project ?? ""}
                placeholder={projects.length ? "Escolher projeto" : "Abra uma pasta raiz"}
                options={projects.map((p) => ({ value: p.path, label: p.name }))}
                onChange={(project) => set({ project })}
              />
            </label>
            <div className="tka-row">
              <label className="tka-field">
                <span>Agente</span>
                <Select
                  ariaLabel="Agente"
                  value={s.provider}
                  options={providers.map((p) => ({ value: p.id, label: p.display_name }))}
                  onChange={(provider) => set({ provider, model: null })}
                />
              </label>
              <label className="tka-field">
                <span>Modelo</span>
                <Select
                  ariaLabel="Modelo"
                  value={s.model ?? ""}
                  options={modelOptions}
                  onOpen={loadModels}
                  note={modelNote}
                  popWidth={260}
                  onChange={(m) => set({ model: m || null })}
                />
              </label>
            </div>
            <div className="tka-field">
              <span>Modo de trabalho</span>
              <Seg<"" | WorkMode>
                label="Modo de trabalho"
                value={s.mode ?? ""}
                onChange={(m) => set({ mode: m || null })}
                options={[
                  { value: "", label: "Padrão do projeto" },
                  { value: "Isolated", label: "Isolado" },
                  { value: "Direct", label: "Direto na pasta" },
                ]}
              />
            </div>
          </>
        )}
        <label className="tka-field">
          <span>{agent ? "Pedido" : "Texto do lembrete"}</span>
          <textarea
            rows={agent ? 6 : 3}
            value={s.prompt}
            onChange={(e) => set({ prompt: e.target.value })}
            placeholder={agent ? "Rode os testes e me diga o que quebrou desde ontem." : "Conferir o backup."}
          />
        </label>
        <div className="tka-field">
          <span>Quando</span>
          <Select ariaLabel="Recorrência" value={d.recKind} options={REC_OPTIONS} onChange={(v) => setD((x) => ({ ...x, recKind: v as RecKind }))} />
        </div>
        {d.recKind === "Once" && (
          <label className="tka-field">
            <span>Data e hora</span>
            <input type="datetime-local" value={d.onceAt} onChange={(e) => setD((x) => ({ ...x, onceAt: e.target.value }))} />
          </label>
        )}
        {d.recKind === "Weekly" && (
          <div className="tka-field">
            <span>Dias</span>
            <div className="tka-days" role="group" aria-label="Dias da semana">
              {WEEKDAYS.map((w, i) => {
                const on = d.days.includes(i);
                return (
                  <button
                    key={w}
                    type="button"
                    aria-pressed={on}
                    className={on ? "on" : undefined}
                    onClick={() => setD((x) => ({ ...x, days: on ? x.days.filter((y) => y !== i) : [...x.days, i] }))}
                  >
                    {w}
                  </button>
                );
              })}
            </div>
          </div>
        )}
        {(d.recKind === "Daily" || d.recKind === "Weekly") && (
          <label className="tka-field tka-narrow">
            <span>Horário</span>
            <input type="time" value={d.time} onChange={(e) => setD((x) => ({ ...x, time: e.target.value }))} />
          </label>
        )}
        {d.recKind === "EveryHours" && (
          <label className="tka-field tka-narrow">
            <span>Intervalo (horas)</span>
            <input type="number" min={1} max={168} value={d.hours} onChange={(e) => setD((x) => ({ ...x, hours: Number(e.target.value) }))} />
          </label>
        )}
        {d.recKind === "Cron" && (
          <label className="tka-field">
            <span>Expressão</span>
            <input className="ui-mono" value={d.cron} onChange={(e) => setD((x) => ({ ...x, cron: e.target.value }))} spellCheck={false} />
            <span className="ui-sm ui-mut">minuto hora dia mês dia-da-semana. Ex.: 0 9 * * 1-5 (dias úteis às 9h).</span>
          </label>
        )}
        <label className="tka-check">
          <input type="checkbox" checked={s.enabled} onChange={(e) => set({ enabled: e.target.checked })} />
          Ligada
        </label>
        <p className="ui-sm ui-mut tka-note">
          Roda só com o Argus aberto, no fuso deste computador. Se ele estiver fechado no horário, roda uma vez ao abrir.
        </p>
        {err && (
          <p className="tka-err" role="alert">
            {err}
          </p>
        )}
        <div className="tka-actions">
          <Btn variant="ghost" onClick={onClose}>
            Cancelar
          </Btn>
          <Btn type="submit" disabled={busy}>
            {busy ? "Salvando…" : s.id ? "Salvar" : "Criar tarefa"}
          </Btn>
        </div>
      </form>
    </aside>
  );
}

export default function TasksArea() {
  const projects = useApp((s) => s.projects);
  const selected = useApp((s) => s.selected);
  const providers = useProviderList();
  const [list, setList] = useState<Schedule[] | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [editing, setEditing] = useState<Schedule | null>(null);
  const [, setTick] = useState(0);

  const reload = useCallback(() => {
    api.schedulesList().then(
      (l) => (setList(l), setErr(null)),
      (e) => setErr(errorMessage(e)),
    );
  }, []);
  useEffect(() => {
    reload();
    const off = onSchedulesChanged(reload);
    // "hoje às…" / "amanhã às…" mudam com o relógio.
    const t = window.setInterval(() => setTick((n) => n + 1), 60_000);
    return () => {
      void off.then((f) => f());
      window.clearInterval(t);
    };
  }, [reload]);

  const act = (p: Promise<unknown>) => p.then(reload, (e) => setErr(errorMessage(e)));
  const openNew = () => setEditing(blank(selected ?? projects[0]?.path ?? null, providers[0]?.id ?? "claude"));
  // Paleta de comandos: "Nova tarefa agendada" (antes ou depois de a área montar).
  useEffect(() => {
    const check = () => {
      if (takeNewTaskRequest()) openNew();
    };
    check();
    return onNewTaskRequest(check);
  });
  const remove = async (s: Schedule) => {
    if (!(await api.systemConfirm("Apagar tarefa", `Apagar a tarefa agendada "${s.name}"? As conversas já criadas continuam.`))) return;
    if (editing?.id === s.id) setEditing(null);
    void act(api.schedulesDelete(s.id));
  };
  const providerName = (id: string) => providers.find((p) => p.id === id)?.display_name ?? id;
  const projectName = (p: string | null) => projects.find((x) => samePath(x.path, p))?.name ?? (p ? baseName(p) : "—");

  const columns: Column<Schedule>[] = [
    {
      key: "name",
      header: "Tarefa",
      render: (s) => (
        <div className="tka-name">
          <span className={s.enabled ? undefined : "ui-mut"}>{s.name}</span>
          <span className="ui-sm ui-mut">{s.kind === "Agent" ? `${projectName(s.project)} · ${providerName(s.provider)}` : "Lembrete"}</span>
        </div>
      ),
    },
    { key: "rec", header: "Quando", render: (s) => <span className="ui-mut">{describeRecurrence(s.recurrence)}</span> },
    {
      key: "next",
      header: "Próxima",
      width: 170,
      render: (s) => (s.enabled && s.next_run ? formatWhen(s.next_run) : <span className="ui-mut">{s.enabled ? "—" : "Desligada"}</span>),
    },
    {
      key: "last",
      header: "Última",
      width: 200,
      render: (s) => {
        const st = runStatusLabel(s.last_status, s.last_late);
        return (
          <div className="tka-last" title={st.detail}>
            <Chip tone={st.tone}>{st.label}</Chip>
            {s.last_task != null && (
              <button
                type="button"
                className="tka-link"
                onClick={(e) => {
                  e.stopPropagation();
                  useWorkbench.getState().openSession(s.last_task!);
                }}
              >
                Abrir conversa
              </button>
            )}
          </div>
        );
      },
    },
    {
      key: "act",
      header: "",
      width: 210,
      render: (s) => (
        <div className="tka-row-actions" onClick={(e) => e.stopPropagation()}>
          <label className="tka-check" title={s.enabled ? "Desligar" : "Ligar"}>
            <input type="checkbox" checked={s.enabled} onChange={(e) => void act(api.schedulesSetEnabled(s.id, e.target.checked))} aria-label={`Ligada: ${s.name}`} />
            Ligada
          </label>
          <Btn small variant="ghost" onClick={() => void act(api.schedulesRunNow(s.id))}>
            Executar agora
          </Btn>
          <Btn small variant="bad" onClick={() => void remove(s)} aria-label={`Apagar ${s.name}`}>
            Apagar
          </Btn>
        </div>
      ),
    },
  ];

  const active = list?.filter((s) => s.enabled).length ?? 0;

  return (
    <div className="ui-area">
      <div className="ui-area-main">
        <AreaHeader
          // Mesmo nome do rail: "Tarefa" sozinho já é a sessão com worktree.
          title="Tarefas agendadas"
          sub={list ? `${list.length} agendada${list.length === 1 ? "" : "s"} · ${active} ligada${active === 1 ? "" : "s"}` : undefined}
          // Com a lista vazia o próprio estado vazio oferece o botão; com o painel aberto, nada a repetir.
          actions={list && list.length > 0 && !editing ? <Btn onClick={openNew}>Nova tarefa agendada</Btn> : undefined}
        />
        {err && (
          <p className="tka-err tka-pad" role="alert">
            {err}
          </p>
        )}
        <div className="tka-scroll">
          {list == null && !err ? (
            <p className="ui-empty tka-pad">Carregando…</p>
          ) : (
            <DataTable
              columns={columns}
              rows={list ?? []}
              rowKey={(s) => s.id}
              selected={editing?.id ?? null}
              onRowClick={(s) => setEditing(s)}
              empty={
                <div className="tka-empty">
                  <span>Nenhuma tarefa agendada. Peça a um agente para rodar algo todo dia, numa data ou de tantas em tantas horas.</span>
                  {!editing && (
                    <Btn small onClick={openNew}>
                      Nova tarefa agendada
                    </Btn>
                  )}
                </div>
              }
            />
          )}
        </div>
        <AreaStatus>roda só com o Argus aberto</AreaStatus>
      </div>
      {editing && (
        <Editor
          key={editing.id || "novo"}
          initial={editing}
          providers={providers}
          onClose={() => setEditing(null)}
          onSaved={() => {
            setEditing(null);
            reload();
          }}
        />
      )}
    </div>
  );
}
