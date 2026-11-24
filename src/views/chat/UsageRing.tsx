/**
 * Bolinha de uso na barra do compositor: um anel que enche com a janela mais
 * usada. Mostra dois tipos de janela:
 *
 * - as informadas pela CLI (`rate_limit_event`), a cada turno;
 * - as de 5 horas calculadas pelo plugin `usage-window` do OpenCode, lidas do
 *   `state.json` dele. Nessas, o limite pode ser ajustado aqui (equivalente ao
 *   `set_limit` e ao `reset_samples` da ferramenta do plugin).
 *
 * O estado do plugin muda a cada resposta da CLI, sem evento para o Argus.
 * Por isso a lista é relida ao abrir o popover e a cada 30 segundos.
 */
import { useCallback, useEffect, useRef, useState, type FormEvent } from "react";
import { api, errorMessage, formatIntPt, onOrch, variantOf, type RateLimitInfo, type UsageWindowInfo } from "@/api";
import { Popover } from "@/shell/Popover";

const WINDOW_LABEL: Record<string, string> = {
  five_hour: "Limite de 5 horas",
  seven_day: "Semanal · todos os modelos",
  seven_day_opus: "Semanal · Opus",
  seven_day_sonnet: "Semanal · Sonnet",
};
const ORDER = ["five_hour", "seven_day", "seven_day_opus", "seven_day_sonnet"];

type BaseRow = {
  id: string;
  name: string;
  pct: number | null;
  resetAtSec: number | null;
  status: string;
};
type CliRow = BaseRow & { kind: "cli" };
type PluginRow = BaseRow & { kind: "plugin"; info: UsageWindowInfo };
type Row = CliRow | PluginRow;

function tone(u: number, status: string): "ok" | "warn" | "err" {
  if (status === "rejected" || u >= 0.9) return "err";
  if (u >= 0.7 || status === "allowed_warning") return "warn";
  return "ok";
}

/** "Reinicia às 18:00" (hoje) ou "Reinicia ter., 10:00". */
function resetLabel(sec: number | null): string {
  if (sec == null) return "";
  const d = new Date(sec * 1000);
  const hm = d.toLocaleTimeString("pt-BR", { hour: "2-digit", minute: "2-digit" });
  if (d.toDateString() === new Date().toDateString()) return `Reinicia às ${hm}`;
  return `Reinicia ${d.toLocaleDateString("pt-BR", { weekday: "short" })}, ${hm}`;
}

/** Ajustes manuais de uma janela do plugin, dentro do popover. */
function PluginControls({ info, changed }: { info: UsageWindowInfo; changed: () => void }) {
  const [editing, setEditing] = useState(false);
  const [tokens, setTokens] = useState("");
  const [requests, setRequests] = useState("");
  const [confirming, setConfirming] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");

  const startEdit = () => {
    setTokens(info.limit_tokens != null ? String(info.limit_tokens) : "");
    setRequests(info.limit_requests != null ? String(info.limit_requests) : "");
    setEditing(true);
    setConfirming(false);
    setError("");
  };

  const save = async (e: FormEvent) => {
    e.preventDefault();
    const t = tokens.trim() ? Number(tokens) : null;
    const r = requests.trim() ? Number(requests) : null;
    if (
      (t != null && (!Number.isFinite(t) || t <= 0)) ||
      (r != null && (!Number.isFinite(r) || r <= 0))
    ) {
      setError("Use números maiores que zero.");
      return;
    }
    setBusy(true);
    setError("");
    try {
      await api.usageWindowSetLimit(info.key, t, r);
      setEditing(false);
      changed();
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  const clear = async () => {
    setBusy(true);
    setError("");
    try {
      await api.usageWindowResetSamples(info.key);
      setConfirming(false);
      changed();
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  const limit = `${info.limit_tokens != null ? formatIntPt(info.limit_tokens) : "?"} tokens / ${
    info.limit_requests != null ? formatIntPt(info.limit_requests) : "?"
  } reqs`;
  const source = info.calibration === "manual" ? "limite manual" : `${info.samples} amostra(s)`;

  return (
    <div className="ur-ctl">
      <div className="ur-detail">
        {formatIntPt(info.tokens)} tokens · {formatIntPt(info.requests)} reqs · limite {limit} ({source})
      </div>
      {editing ? (
        <form className="ur-form" onSubmit={(e) => void save(e)}>
          <label>
            Tokens
            <input
              type="number"
              min={1}
              step={1}
              value={tokens}
              onChange={(e) => setTokens(e.target.value)}
              aria-label={`Limite de tokens para ${info.model}`}
            />
          </label>
          <label>
            Requisições
            <input
              type="number"
              min={1}
              step={1}
              value={requests}
              onChange={(e) => setRequests(e.target.value)}
              aria-label={`Limite de requisições para ${info.model}`}
            />
          </label>
          <div className="ur-actions">
            <button type="submit" disabled={busy}>
              Salvar
            </button>
            <button type="button" onClick={() => { setEditing(false); setError(""); }}>
              Cancelar
            </button>
          </div>
        </form>
      ) : (
        <div className="ur-actions">
          <button type="button" onClick={startEdit}>
            Ajustar limite
          </button>
          {confirming ? (
            <>
              <button type="button" onClick={() => void clear()} disabled={busy}>
                Confirmar limpeza
              </button>
              <button type="button" onClick={() => setConfirming(false)}>
                Cancelar
              </button>
            </>
          ) : (
            <button type="button" onClick={() => { setConfirming(true); setError(""); }}>
              Limpar amostras
            </button>
          )}
        </div>
      )}
      {error && (
        <p className="ur-err" role="alert">
          {error}
        </p>
      )}
    </div>
  );
}

export function UsageRing() {
  const [limits, setLimits] = useState<RateLimitInfo[]>([]);
  const [windows, setWindows] = useState<UsageWindowInfo[]>([]);
  const [open, setOpen] = useState(false);
  const btn = useRef<HTMLButtonElement>(null);
  const alive = useRef(true);

  const refresh = useCallback(() => {
    void api.rateLimits().then((l) => alive.current && setLimits(l), () => undefined);
    void api.usageWindows().then((w) => alive.current && setWindows(w), () => undefined);
  }, []);

  useEffect(() => {
    alive.current = true;
    refresh();
    const timer = window.setInterval(refresh, 30_000);
    const off = onOrch((ev) => {
      if (!("Agent" in ev)) return;
      const { kind, data } = variantOf(ev.Agent.event);
      if (kind !== "RateLimit") return;
      const info = data as RateLimitInfo;
      setLimits((cur) => [...cur.filter((x) => x.window !== info.window), info]);
    });
    return () => {
      alive.current = false;
      window.clearInterval(timer);
      void off.then((f) => f()).catch(() => undefined);
    };
  }, [refresh]);

  // Só janelas que a CLI informou de verdade e cujo ciclo ainda não virou: um
  // valor de ciclo vencido não vale mais, e janela nenhuma é inventada aqui.
  const nowSec = Date.now() / 1000;
  const cli: CliRow[] = limits
    .filter((l) => l.utilization != null && !(l.resets_at != null && l.resets_at <= nowSec))
    .sort((a, b) => ((ORDER.indexOf(a.window) + 99) % 99) - ((ORDER.indexOf(b.window) + 99) % 99))
    .map((l) => ({
      kind: "cli",
      id: `cli:${l.window}`,
      name: WINDOW_LABEL[l.window] ?? l.window,
      pct: Math.round((l.utilization ?? 0) * 100),
      resetAtSec: l.resets_at,
      status: l.status,
    }));
  const plugin: PluginRow[] = windows.map((w) => ({
    kind: "plugin",
    id: `plugin:${w.key}`,
    name: `${w.provider ? `${w.provider} · ` : ""}${w.model}${w.is_free ? " · free" : ""}`,
    pct: w.percent,
    resetAtSec: Math.floor(w.resets_at_ms / 1000),
    status: w.limit_hit_at_ms != null ? "rejected" : "allowed",
    info: w,
  }));
  const all: Row[] = [...cli, ...plugin];
  if (all.length === 0) return null;

  const top = all.reduce((m, l) => ((l.pct ?? -1) > (m.pct ?? -1) ? l : m), all[0]);
  const u = Math.min(1, Math.max(0, top.pct ?? 0) / 100);
  const t = tone(u, top.status);
  const R = 6.5;
  const C = 2 * Math.PI * R;
  const summary =
    all.map((l) => `${l.name}: ${l.pct == null ? "sem calibração" : `${l.pct}%`}`).join(" · ") +
    " · clique para detalhes";

  return (
    <span className="ur">
      <button
        ref={btn}
        type="button"
        className={`ur-btn ${t}`}
        aria-label={`Uso do plano: ${summary}`}
        title={summary}
        aria-expanded={open}
        aria-haspopup="dialog"
        onClick={() => { setOpen((v) => !v); refresh(); }}
      >
        <svg width="16" height="16" viewBox="0 0 16 16" aria-hidden="true">
          <circle cx="8" cy="8" r={R} className="ur-track" />
          <circle cx="8" cy="8" r={R} className="ur-fill" strokeDasharray={C} strokeDashoffset={C * (1 - u)} transform="rotate(-90 8 8)" />
        </svg>
      </button>
      <Popover anchor={btn} open={open} onClose={() => setOpen(false)} ariaLabel="Seus limites de uso" width={360} className="ur-pop">
        <div className="ur-head">Seus limites de uso</div>
        {all.map((row) => {
          const shown = row.pct;
          const lt = tone((shown ?? 0) / 100, row.status);
          return (
            <div key={row.id} className={`ur-row${row.kind === "plugin" ? " ur-plugin" : ""}`}>
              <div className="ur-line">
                <span className="ur-name">{row.name}</span>
                <span className="ur-meta">
                  {resetLabel(row.resetAtSec)}
                  <b>{shown == null ? "—" : `${shown}%`}</b>
                </span>
              </div>
              {shown == null ? (
                <span className="ur-nocal">Sem calibração: defina um limite para ver o %.</span>
              ) : (
                <div className="ur-bar" role="progressbar" aria-valuemin={0} aria-valuemax={100} aria-valuenow={shown} aria-label={row.name}>
                  <i className={lt} style={{ transform: `scaleX(${Math.min(1, shown / 100)})` }} />
                </div>
              )}
              {row.kind === "plugin" && <PluginControls info={row.info} changed={refresh} />}
            </div>
          );
        })}
        <p className="ur-note">
          As linhas da CLI vêm do próprio provedor. As janelas de 5 horas vêm do plugin `usage-window` do OpenCode;
          o percentual é uma estimativa local, não a cota oficial do provider.
        </p>
      </Popover>
    </span>
  );
}
