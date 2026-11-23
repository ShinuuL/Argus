/**
 * Tela 07 · Servidores e portas (docs/mockups/argus-telas.html, #s7).
 * "Dos meus projetos" usa os servidores já detectados (useApp.servers). As abas
 * de portas dependem do módulo de portas em Rust, que ainda não existe: mostram
 * um estado vazio honesto.
 */
import { useEffect, useMemo, useState } from "react";
import { api, baseName, errorMessage, formatUptime, ownerLabel, sameOwner, type ServerInfo } from "@/api";
import { useApp, useLayout } from "@/store";
import { DevServersSection } from "@/views/servers/devservers";
import { useOwnersUsage } from "@/views/servers";
import { AreaHeader, Btn, Card, Chip, DataTable, Empty, Tab, Tabs, type Column } from "@/ui";
import { Icon } from "@/shell/icons";
import "./servers-area.css";

type PortTab = "mine" | "all" | "conflicts" | "reserved";
const keyOf = (s: ServerInfo) => `${s.pid}-${s.port}`;

/** Por que um servidor fora da árvore da sessão aparece aqui. */
const FOLDER_HINT =
  "Está rodando na pasta do projeto desta sessão, mas fora da árvore dela. Acontece com agente que executa por um daemon próprio (opencode) e com servidor que você mesmo subiu na pasta.";

function fmtMem(b: number): string {
  if (b >= 1 << 30) return `${(b / (1 << 30)).toFixed(1)} GB`;
  return `${Math.round(b / (1 << 20))} MB`;
}

function useNow(ms: number): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const t = setInterval(() => setNow(Date.now()), ms);
    return () => clearInterval(t);
  }, [ms]);
  return now;
}

const EMPTY_TEXT: Record<Exclude<PortTab, "mine">, string> = {
  all: "A lista de todas as portas da máquina chega com o módulo de portas.",
  conflicts: "A detecção de duas escutas na mesma porta chega com o módulo de portas.",
  reserved: "As faixas reservadas pelo Hyper-V/WSL chegam com o módulo de portas.",
};

function Detail({ server, memory, onError }: { server: ServerInfo; memory: number | null; onError(msg: string | null): void }) {
  const [busy, setBusy] = useState<"restart" | "kill" | "tunnel" | null>(null);
  const now = useNow(1000);
  const run = async (kind: "restart" | "kill" | "tunnel", action: () => Promise<unknown>) => {
    setBusy(kind);
    try {
      await action();
      onError(null);
    } catch (e) {
      onError(errorMessage(e));
    } finally {
      setBusy(null);
    }
  };
  return (
    <div className="srva-detail">
      <p className="srva-h">
        :{server.port} · {server.name}
      </p>
      <div className="ui-mut ui-sm ui-mono srva-cmd" title={server.cmdline}>
        {server.cmdline || "(linha de comando indisponível)"}
      </div>
      <div className="srva-cards">
        <div className="ui-card">
          <div className="ui-mut ui-sm">PID</div>
          <span className="ui-mono">{server.pid}</span>
        </div>
        <div className="ui-card" title="Memória somada do dono (agente, filhos e servidores)">
          <div className="ui-mut ui-sm">Memória</div>
          {memory != null ? fmtMem(memory) : "—"}
        </div>
        <div className="ui-card">
          <div className="ui-mut ui-sm">No ar há</div>
          {formatUptime(server.started_at, now)}
        </div>
        <div className="ui-card" title={server.source === "folder" ? FOLDER_HINT : "Descende do processo que o Argus lançou."}>
          <div className="ui-mut ui-sm">Dono</div>
          {ownerLabel(server.owner)}
          {server.source === "folder" && <span className="ui-mut ui-sm"> · pela pasta</span>}
        </div>
      </div>
      {server.exposure && (
        <div className="srva-exposed" role="status">
          <div>
            <b>Exposto na internet</b> pelo {server.exposure.label}
            {server.exposure.url ? (
              <>
                {" · "}
                <button
                  type="button"
                  className="srva-exposed-link"
                  onClick={() => api.systemOpenUrl(server.exposure!.url!).then(() => onError(null), (e) => onError(errorMessage(e)))}
                >
                  {server.exposure.url}
                </button>
              </>
            ) : (
              " · endereço público desconhecido"
            )}
            <div className="ui-mut ui-sm">Quem tiver o endereço alcança esta porta da sua máquina, sem senha.</div>
          </div>
          <Btn
            variant="bad"
            disabled={busy !== null}
            onClick={() =>
              void run("tunnel", async () => {
                useApp.setState({ servers: await api.serverKill(server.exposure!.pid) });
              })
            }
          >
            {busy === "tunnel" ? "Fechando…" : "Fechar o túnel"}
          </Btn>
        </div>
      )}
      <Btn variant="ghost" onClick={() => api.systemOpenUrl(server.url).then(() => onError(null), (e) => onError(errorMessage(e)))}>
        Abrir no navegador
      </Btn>
      <Btn variant="ghost" disabled={!server.cmdline || busy !== null} onClick={() => void run("restart", () => api.serverRestart(server))}>
        {busy === "restart" ? "Reiniciando…" : "Reiniciar"}
      </Btn>
      <Btn
        variant="bad"
        disabled={busy !== null}
        onClick={() =>
          void run("kill", async () => {
            useApp.setState({ servers: await api.serverKill(server.pid) });
          })
        }
      >
        {busy === "kill" ? "Encerrando…" : "Encerrar árvore"}
      </Btn>
      <div className="ui-mut ui-sm">Bloquear no firewall pede admin (helper elevado) e chega com o módulo de portas.</div>
    </div>
  );
}

export default function ServersArea() {
  const servers = useApp((s) => s.servers);
  const hasProjects = useApp((s) => s.projects.length > 0);
  const filter = useLayout((s) => s.serversFilter);
  const openPanel = useLayout((s) => s.openServersPanel);
  const [tab, setTab] = useState<PortTab>("mine");
  const [tcp, setTcp] = useState(true);
  const [udp, setUdp] = useState(false);
  const [listen, setListen] = useState(true);
  const [q, setQ] = useState("");
  const [sel, setSel] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const usage = useOwnersUsage();
  const usageOf = (s: ServerInfo) => usage.find((u) => sameOwner(u.owner, s.owner));

  // Os servidores de dev detectados são sempre TCP em escuta.
  const rows = useMemo(() => {
    const term = q.trim().toLowerCase();
    if (!tcp || !listen) return [];
    return servers
      .filter((s) => !filter || sameOwner(s.owner, filter))
      .filter((s) => !term || `${s.port} ${s.name} ${s.cmdline}`.toLowerCase().includes(term))
      .sort((a, b) => a.port - b.port);
  }, [servers, filter, tcp, listen, q]);

  const selected = rows.find((s) => keyOf(s) === sel) ?? rows[0] ?? null;

  /*
   * Lista vazia e nada escolhido: a tela inteira vira o estado vazio. Os três
   * filtros ligados e o campo "filtrar porta ou processo" só aparecem quando
   * há o que refinar — refino antes de existir conteúdo é controle pedindo
   * trabalho sem oferecer resultado.
   */
  const semServidor = tab === "mine" && !filter && !q.trim() && servers.length === 0;

  const columns: Column<ServerInfo>[] = [
    { key: "port", header: "Porta", width: 90, render: (s) => <span className="ui-mono">{s.port}</span> },
    { key: "proto", header: "Proto", width: 60, render: () => "TCP" },
    { key: "proc", header: "Processo", render: (s) => <span title={s.cmdline}>{s.name}</span> },
    { key: "proj", header: "Projeto", render: (s) => (s.cwd ? baseName(s.cwd) : ownerLabel(s.owner)) },
    {
      key: "state",
      header: "Estado",
      width: 150,
      render: (s) => (
        <>
          <Chip tone="ok">no ar</Chip>
          {s.exposure && (
            <Chip tone="warn" title={`Publicado na internet pelo ${s.exposure.label}${s.exposure.url ? `: ${s.exposure.url}` : ""}`}>
              exposto
            </Chip>
          )}
        </>
      ),
    },
    {
      key: "cpu",
      header: "CPU",
      right: true,
      width: 60,
      render: (s) => {
        const u = usageOf(s);
        return <span title="CPU somada do dono">{u ? `${Math.round(u.cpu)}%` : "—"}</span>;
      },
    },
  ];

  return (
    <div className="ui-area srva">
      <div className="ui-area-main">
        {semServidor ? (
          <>
            <AreaHeader title="Servidores" sub="nenhum em execução" />
            <Empty
              icon={<Icon name="server" size={20} />}
              title="Nenhum servidor em execução"
              action={
                hasProjects ? undefined : <Btn onClick={() => void useApp.getState().pickRoot()}>Abrir pasta raiz</Btn>
              }
            >
              Quando um agente subir um servidor — ou quando um já estiver rodando na pasta do
              projeto — ele aparece aqui, com a porta, o processo e o botão de abrir, parar ou
              expor por túnel.
            </Empty>
            {hasProjects && (
              <div className="ui-pad">
                <Card title="Servidores de dev dos projetos">
                  <DevServersSection />
                </Card>
              </div>
            )}
          </>
        ) : (
        <>
        <AreaHeader title="Servidores" sub={`${servers.length} em execução`} />
        <div className="srva-bar">
          <Tabs label="Visões de portas" className="srva-tabs">
            <Tab on={tab === "mine"} count={servers.length} onClick={() => setTab("mine")}>
              Dos meus projetos
            </Tab>
            <Tab on={tab === "all"} onClick={() => setTab("all")}>
              Todas as portas
            </Tab>
            <Tab on={tab === "conflicts"} onClick={() => setTab("conflicts")}>
              Conflitos
            </Tab>
            <Tab on={tab === "reserved"} onClick={() => setTab("reserved")}>
              Reservadas
            </Tab>
          </Tabs>
          <span className="ui-sp" />
          {filter && (
            <Chip tone="info" onClick={() => openPanel(null)} title="Mostrar os servidores de todas as sessões">
              {ownerLabel(filter)} ✕
            </Chip>
          )}
          <input
            className="srva-q"
            placeholder="Filtrar porta ou processo"
            value={q}
            onChange={(e) => setQ(e.target.value)}
            aria-label="Filtrar servidores"
          />
          <Chip className={tcp ? "srva-on" : ""} onClick={() => setTcp(!tcp)}>
            TCP
          </Chip>
          <Chip className={udp ? "srva-on" : ""} onClick={() => setUdp(!udp)} title="UDP depende do módulo de portas">
            UDP
          </Chip>
          <Chip className={listen ? "srva-on" : ""} onClick={() => setListen(!listen)}>
            Escutando
          </Chip>
        </div>
        {error && (
          <div className="srva-err" role="alert">
            {error}
          </div>
        )}
        {tab === "mine" ? (
          <div className="srva-scroll">
            <DataTable
              columns={columns}
              rows={rows}
              rowKey={keyOf}
              selected={selected ? keyOf(selected) : undefined}
              onRowClick={(s) => setSel(keyOf(s))}
              /*
               * O `empty` é montado sempre, mesmo com linhas na tabela — então
               * aqui não cabe `filter!`: sem filtro, `ownerLabel(null)` derruba
               * a área inteira. Quem não tem filtro tem o campo de busca.
               */
              empty={
                <Empty
                  icon={<Icon name="filter" size={20} />}
                  title={filter ? `Nenhum servidor em ${ownerLabel(filter)}` : "Nenhum servidor com esse filtro"}
                >
                  {filter
                    ? "Os servidores de outras sessões continuam na lista sem este filtro."
                    : "Limpe a busca ou religue TCP e Escutando para ver a lista inteira."}
                </Empty>
              }
            />
            {!filter && hasProjects && (
              <div className="ui-pad">
                <Card title="Servidores de dev dos projetos">
                  <DevServersSection />
                </Card>
              </div>
            )}
          </div>
        ) : (
          <div className="ui-empty">
            <b>Módulo de portas em construção</b>
            <div className="ui-mut ui-sm">{EMPTY_TEXT[tab]}</div>
          </div>
        )}
        </>
        )}
      </div>
      {/* O painel de detalhe só ocupa espaço quando há um servidor escolhido. */}
      {tab === "mine" && selected && (
        <aside className="srva-panel" aria-label="Detalhe do servidor">
          <Detail server={selected} memory={usageOf(selected)?.memory ?? null} onError={setError} />
        </aside>
      )}
    </div>
  );
}
