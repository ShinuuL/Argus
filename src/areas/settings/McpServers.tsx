/**
 * Configurações › Ferramentas MCP › Servidores da CLI: o que `claude mcp list`
 * mostra (com o estado de cada um), adicionar, remover e entrar (OAuth, num
 * terminal do Argus, porque o login é interativo).
 */
import { useEffect, useState } from "react";
import { api, errorMessage, type McpServer, type Plugin } from "@/api";
import { useWorkbench } from "@/store";
import { Chip, type Tone } from "@/ui";
import { Seg } from "@/ui/components";

const STATUS: Record<McpServer["status"], { label: string; tone: Tone }> = {
  connected: { label: "conectado", tone: "ok" },
  needs_auth: { label: "precisa entrar", tone: "warn" },
  failed: { label: "falhou", tone: "bad" },
  unknown: { label: "desconhecido", tone: "info" },
};

type Transport = "stdio" | "http" | "sse";
type Scope = "local" | "user" | "project";

export function McpServers() {
  const [list, setList] = useState<McpServer[] | null>(null);
  const [loading, setLoading] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const [msg, setMsg] = useState<string | null>(null);
  const [adding, setAdding] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);

  const load = () => {
    setLoading(true);
    api.mcpList().then(
      (l) => {
        setList(l);
        setErr(null);
        setLoading(false);
      },
      (e) => {
        setErr(errorMessage(e));
        setLoading(false);
      },
    );
  };
  useEffect(load, []);

  const remove = async (name: string) => {
    if (!window.confirm(`Remover o servidor MCP "${name}" da CLI do Claude?`))
      return;
    setBusy(name);
    try {
      setMsg(await api.mcpRemove(name));
      load();
    } catch (e) {
      setErr(errorMessage(e));
    } finally {
      setBusy(null);
    }
  };

  const login = (name: string) =>
    useWorkbench
      .getState()
      .openTerminal({
        title: `MCP: entrar em ${name}`,
        cwd: null,
        command: { program: "claude", args: ["mcp", "login", name] },
      });

  const local = list?.filter((s) => !s.remote) ?? [];
  const remote = list?.filter((s) => s.remote) ?? [];

  return (
    <section className="mcp" aria-label="Servidores MCP da CLI do Claude">
      <div className="mcp-head">
        <h3>Servidores da CLI do Claude</h3>
        <button type="button" onClick={load} disabled={loading}>
          {loading ? "Conferindo…" : "Conferir de novo"}
        </button>
        <button
          type="button"
          className="primary"
          onClick={() => setAdding(!adding)}
          aria-expanded={adding}
        >
          Adicionar
        </button>
      </div>
      <p className="small muted">
        Valem para todas as conversas do Claude no Argus. A lista confere a
        conexão de cada servidor e pode levar alguns segundos.
      </p>
      {adding && (
        <AddForm
          onDone={(m) => {
            setMsg(m);
            setAdding(false);
            load();
          }}
        />
      )}
      {err && <div className="err small">{err}</div>}
      {msg && <div className="small muted mcp-msg">{msg}</div>}
      {list == null && !err && (
        <p className="small muted">Conferindo os servidores…</p>
      )}
      {list && (
        <>
          <McpTable
            title="Seus servidores"
            rows={local}
            empty="Nenhum servidor adicionado na CLI."
            busy={busy}
            onRemove={remove}
            onLogin={login}
          />
          <McpTable
            title="Conectores da conta (claude.ai)"
            rows={remote}
            empty="Nenhum conector da conta."
            busy={busy}
            onLogin={login}
          />
        </>
      )}
    </section>
  );
}

function McpTable({
  title,
  rows,
  empty,
  busy,
  onRemove,
  onLogin,
}: {
  title: string;
  rows: McpServer[];
  empty: string;
  busy: string | null;
  onRemove?: (name: string) => void;
  onLogin: (name: string) => void;
}) {
  return (
    <>
      <h4 className="mcp-sub">{title}</h4>
      {rows.length === 0 ? (
        <p className="small muted">{empty}</p>
      ) : (
        <table className="ui-t">
          <thead>
            <tr>
              <th>Servidor</th>
              <th>Endereço ou comando</th>
              <th style={{ width: 130 }}>Estado</th>
              <th style={{ width: 150 }} />
            </tr>
          </thead>
          <tbody>
            {rows.map((s) => {
              const st = STATUS[s.status];
              return (
                <tr key={s.name}>
                  <td className="ui-mono">
                    {s.name.replace(/^claude\.ai /, "")}
                  </td>
                  <td className="ui-mut ui-mono mcp-target" title={s.target}>
                    {s.target}
                  </td>
                  <td>
                    <Chip tone={st.tone} title={s.status_text}>
                      {st.label}
                    </Chip>
                  </td>
                  <td>
                    <div className="mcp-actions">
                      {s.status === "needs_auth" && !s.remote && (
                        <button type="button" onClick={() => onLogin(s.name)}>
                          Entrar
                        </button>
                      )}
                      {s.status === "needs_auth" && s.remote && (
                        <span className="small muted">
                          entrar pelo claude.ai
                        </span>
                      )}
                      {onRemove && s.name.startsWith("plugin:") && (
                        <span className="small muted">do plugin</span>
                      )}
                      {onRemove && !s.name.startsWith("plugin:") && (
                        <button
                          type="button"
                          disabled={busy === s.name}
                          onClick={() => onRemove(s.name)}
                        >
                          {busy === s.name ? "Removendo…" : "Remover"}
                        </button>
                      )}
                    </div>
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      )}
    </>
  );
}

function AddForm({ onDone }: { onDone: (msg: string) => void }) {
  const [name, setName] = useState("");
  const [transport, setTransport] = useState<Transport>("stdio");
  const [target, setTarget] = useState("");
  const [scope, setScope] = useState<Scope>("user");
  const [env, setEnv] = useState("");
  const [err, setErr] = useState<string | null>(null);
  const [sending, setSending] = useState(false);

  const submit = async () => {
    setSending(true);
    setErr(null);
    // stdio: o primeiro termo é o comando, o resto são argumentos.
    const parts = target.trim().split(/\s+/);
    try {
      const out = await api.mcpAdd({
        name: name.trim(),
        transport,
        target: transport === "stdio" ? (parts[0] ?? "") : target.trim(),
        args: transport === "stdio" ? parts.slice(1) : [],
        scope,
        env: env
          .split(/\n/)
          .map((l) => l.trim())
          .filter(Boolean),
      });
      onDone(out || `Servidor ${name} adicionado.`);
    } catch (e) {
      setErr(errorMessage(e));
    } finally {
      setSending(false);
    }
  };

  return (
    <form
      className="mcp-form"
      onSubmit={(e) => {
        e.preventDefault();
        void submit();
      }}
    >
      <label>
        Nome
        <input
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="ex.: sentry"
          required
          pattern="[A-Za-z0-9._\-]+"
        />
      </label>
      <div className="mcp-field">
        <span>Tipo</span>
        <Seg<Transport>
          label="Tipo de conexão"
          value={transport}
          onChange={setTransport}
          options={[
            { value: "stdio", label: "Comando local" },
            { value: "http", label: "HTTP" },
            { value: "sse", label: "SSE" },
          ]}
        />
      </div>
      <label>
        {transport === "stdio" ? "Comando" : "URL"}
        <input
          className="ui-mono"
          value={target}
          onChange={(e) => setTarget(e.target.value)}
          placeholder={
            transport === "stdio"
              ? "npx -y @modelcontextprotocol/server-filesystem C:\\pasta"
              : "https://mcp.exemplo.com/mcp"
          }
          required
        />
      </label>
      <div className="mcp-field">
        <span>Vale para</span>
        <Seg<Scope>
          label="Escopo"
          value={scope}
          onChange={setScope}
          options={[
            { value: "user", label: "Todos os projetos" },
            { value: "local", label: "Só a pasta atual" },
            { value: "project", label: "Projeto (.mcp.json)" },
          ]}
        />
      </div>
      {transport === "stdio" && (
        <label>
          Variáveis de ambiente (uma por linha, CHAVE=valor)
          <textarea
            rows={2}
            className="ui-mono"
            value={env}
            onChange={(e) => setEnv(e.target.value)}
            placeholder="API_KEY=..."
          />
        </label>
      )}
      <p className="small muted">
        Chaves e senhas ficam na configuração da CLI do Claude, em texto.
        Prefira o login (OAuth) quando o servidor oferecer.
      </p>
      {err && <div className="err small">{err}</div>}
      <div className="mcp-actions">
        <button
          type="submit"
          className="primary"
          disabled={sending || !name.trim() || !target.trim()}
        >
          {sending ? "Adicionando…" : "Adicionar servidor"}
        </button>
      </div>
    </form>
  );
}

/** Plugins da CLI do Claude: lista, ativar e desativar (`claude plugin list/enable/disable`). */
export function Plugins() {
  const [list, setList] = useState<Plugin[] | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const load = () =>
    api.pluginsList().then(
      (l) => {
        setList(l);
        setErr(null);
      },
      (e) => setErr(errorMessage(e)),
    );
  useEffect(() => {
    void load();
  }, []);
  const toggle = async (p: Plugin) => {
    setBusy(p.id);
    try {
      await api.pluginSet(p.id, !p.enabled);
      await load();
    } catch (e) {
      setErr(errorMessage(e));
    } finally {
      setBusy(null);
    }
  };
  return (
    <section className="mcp" aria-label="Plugins da CLI do Claude">
      <div className="mcp-head">
        <h3>Plugins da CLI do Claude</h3>
        <button type="button" onClick={() => void load()}>
          Conferir de novo
        </button>
      </div>
      <p className="small muted">
        Plugins trazem skills, comandos, agentes e servidores MCP. Para instalar
        novos, use <code>claude plugin install</code> num terminal.
      </p>
      {err && <div className="err small">{err}</div>}
      {list == null && !err && <p className="small muted">Lendo os plugins…</p>}
      {list && list.length === 0 && (
        <p className="small muted">Nenhum plugin instalado.</p>
      )}
      {list && list.length > 0 && (
        <table className="ui-t">
          <thead>
            <tr>
              <th>Plugin</th>
              <th style={{ width: 90 }}>Versão</th>
              <th style={{ width: 140 }}>Escopo</th>
              <th>MCP</th>
              <th style={{ width: 120 }} />
            </tr>
          </thead>
          <tbody>
            {list.map((p) => (
              <tr key={p.id}>
                <td className="ui-mono">{p.id}</td>
                <td className="ui-mut ui-mono">{p.version || "—"}</td>
                <td className="ui-mut">{SCOPE_LABEL[p.scope] ?? p.scope}</td>
                <td className="ui-mut ui-mono">
                  {p.mcp_servers.join(", ") || "—"}
                </td>
                <td>
                  <div className="mcp-actions">
                    <button
                      type="button"
                      disabled={busy === p.id}
                      onClick={() => void toggle(p)}
                      aria-pressed={p.enabled}
                    >
                      {busy === p.id ? "…" : p.enabled ? "Desativar" : "Ativar"}
                    </button>
                  </div>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </section>
  );
}

const SCOPE_LABEL: Record<string, string> = {
  user: "todos os projetos",
  project: "projeto",
  local: "pasta",
};
