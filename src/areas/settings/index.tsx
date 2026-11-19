/**
 * Área "Configurações" em tela cheia (tela 12 do mockup docs/mockups/argus-telas.html):
 * menu lateral em 5 grupos com chips de estado, busca de configuração e a página
 * Permissões como tabela de ações. Reaproveita as views antigas e as APIs existentes.
 */
import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { api, errorMessage, samePath, type AutostartInfo, type Path } from "@/api";
import { THEMES, useApp, useTheme, useWorkbench } from "@/store";
import { Chip, SideHeading, SideItem, SideList, type Tone, AreaStatus } from "@/ui";
import ProvidersView, { useProviders } from "@/views/agents";
import AutoWorkPanel from "@/views/autowork";
import DepsView from "@/views/deps";
import RemoteView from "@/views/remote";
import PetPage from "@/views/pet/Page";
import PlayerPage from "@/views/player/Page";
import { Select } from "@/shell/Select";
import AgentDefsPage from "./AgentDefs";
import PermissionsPage from "./Permissions";
import { PageBody } from "./PageBody";
import "@/views/settings/settings.css";
import "./settings-area.css";

type Page =
  | "aparencia"
  | "atalhos"
  | "inicializacao"
  | "provedores"
  | "subagentes"
  | "contexto"
  | "permissoes"
  | "mcp"
  | "deps"
  | "auxiliares"
  | "remoto"
  | "auditoria"
  | "pet"
  | "player"
  | "autowork";

interface Item {
  id: Page;
  label: string;
  keywords: string;
}

const GROUPS: { label: string; items: Item[] }[] = [
  {
    label: "Você",
    items: [
      { id: "aparencia", label: "Aparência", keywords: "tema cores ardósia grafite noite oceano" },
      { id: "atalhos", label: "Atalhos", keywords: "teclado ctrl" },
    ],
  },
  {
    label: "Agentes",
    items: [
      { id: "provedores", label: "Provedores", keywords: "claude codex gemini opencode login modelo" },
      { id: "subagentes", label: "Subagentes", keywords: "agentes skills frontmatter AGENTS.md claude agents arquivo" },
      { id: "contexto", label: "Contexto", keywords: "compactar compact tokens janela resumo economia" },
      { id: "permissoes", label: "Permissões", keywords: "política comandos git push permitir bloquear modo trabalho" },
      { id: "mcp", label: "Ferramentas MCP", keywords: "celular screen-mirror" },
    ],
  },
  {
    label: "Este computador",
    items: [
      { id: "inicializacao", label: "Inicialização", keywords: "iniciar com o windows login abrir sozinho minimizado tarefas agendadas" },
      { id: "deps", label: "Dependências", keywords: "node npm instalar" },
      { id: "auxiliares", label: "Auxiliares", keywords: "helper elevado uac" },
      { id: "remoto", label: "Acesso remoto", keywords: "celular qr pareamento rede" },
    ],
  },
  { label: "Registro", items: [{ id: "auditoria", label: "Auditoria", keywords: "audit log decisões" }] },
  {
    label: "Extras",
    items: [
      { id: "pet", label: "Pet", keywords: "mascote" },
      { id: "player", label: "Player", keywords: "música mídia" },
      { id: "autowork", label: "Autowork", keywords: "automático trabalho" },
    ],
  },
];

const ALL = GROUPS.flatMap((g) => g.items);

/** Seção antiga do store (openSettings) → página nova. */
const FROM_SECTION: Record<string, Page> = { theme: "aparencia", agents: "permissoes", providers: "provedores", project: "permissoes" };

const fold = (s: string) => s.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();

/**
 * Chips de estado do menu, a partir de dados reais. O chip do acesso remoto
 * também acompanha o ligar/desligar feito dentro da página (`setRemote`), senão
 * ficaria preso ao estado do momento em que as Configurações abriram.
 */
function useSideStatus(project: Path | null) {
  const providers = useProviders();
  const [deps, setDeps] = useState<number | null>(null);
  const [remote, setRemote] = useState<boolean | null>(null);
  const [mcp, setMcp] = useState<boolean | null>(null);
  useEffect(() => {
    let alive = true;
    api.remoteInfo().then((i) => alive && setRemote(i != null), () => {});
    api.devicesStatus().then((s) => alive && setMcp(s.agentsMcp), () => {});
    return () => void (alive = false);
  }, []);
  useEffect(() => {
    let alive = true;
    setDeps(null);
    if (project) api.depsAnalyze(project).then((l) => alive && setDeps(l.length), () => {});
    return () => void (alive = false);
  }, [project]);

  const chip = (text: string, tone?: Tone) => <Chip tone={tone}>{text}</Chip>;
  const meta: Partial<Record<Page, ReactNode>> = {};
  if (providers.length) meta.provedores = chip(String(providers.length), "ok");
  if (mcp != null) meta.mcp = chip(mcp ? "1" : "0");
  if (deps != null) meta.deps = deps ? chip(`${deps} falta${deps > 1 ? "m" : ""}`, "warn") : chip("ok", "ok");
  if (remote != null) meta.remoto = remote ? chip("ligado", "ok") : chip("desligado");
  return { meta, setRemote };
}

function ThemePage() {
  const theme = useTheme((s) => s.theme);
  const setTheme = useTheme((s) => s.setTheme);
  return (
    <PageBody title="Aparência" lead="Tema de cores. Vale para toda a interface, o editor e os terminais. A escolha fica lembrada.">
      <div className="set-themes" role="radiogroup" aria-label="Tema de cores">
        {THEMES.map((t) => (
          <button
            key={t.id}
            role="radio"
            aria-checked={theme === t.id}
            className={theme === t.id ? "set-theme on" : "set-theme"}
            title={`Usar o tema ${t.label}`}
            onClick={() => setTheme(t.id)}
          >
            <span className="set-swatch" data-theme={t.id} aria-hidden="true">
              <span className="set-sw-side" />
              <span className="set-sw-main">
                <span className="set-sw-line" />
                <span className="set-sw-line short" />
                <span className="set-sw-btn" />
              </span>
            </span>
            <span className="set-theme-name">
              <b>{t.label}</b>
              <span className="small">{t.hint}</span>
            </span>
          </button>
        ))}
      </div>
    </PageBody>
  );
}

const SHORTCUTS: [string, string][] = [
  ["Ctrl+K", "Abrir a paleta de comandos"],
  ["Ctrl+.", "Mostrar ou esconder o inspetor"],
  ["Ctrl+N", "Nova conversa"],
  ["Ctrl+B", "Mostrar ou esconder a lista de projetos"],
  ["Ctrl+J", "Mostrar ou esconder o painel inferior"],
  ["Ctrl+Tab", "Próxima aba (Ctrl+Shift+Tab volta)"],
  ["Ctrl+W", "Fechar a aba"],
  ["Ctrl+\\", "Dividir o editor"],
  ["/", "Buscar configuração (nesta tela)"],
];

const COMPACT_OPTIONS: { value: number; label: string }[] = [
  { value: 100_000, label: "100 mil tokens" },
  { value: 200_000, label: "200 mil tokens (padrão)" },
  { value: 400_000, label: "400 mil tokens" },
  { value: 0, label: "Nunca" },
];

function ContextPage() {
  const [limit, setLimit] = useState<number | null>(null);
  const [err, setErr] = useState<string | null>(null);
  useEffect(() => {
    api.autocompactGet().then(setLimit, (e) => setErr(errorMessage(e)));
  }, []);
  const choose = (v: number) => {
    setLimit(v);
    api.autocompactSet(v).catch((e) => setErr(errorMessage(e)));
  };
  return (
    <PageBody
      title="Contexto"
      lead="Quando a conversa passa do limite, o Argus compacta antes do próximo turno: o agente segue com um resumo, e cada ferramenta deixa de reler o histórico inteiro. Vale para o Claude Code."
    >
      {err && <div className="err" role="alert">{err}</div>}
      <div className="set-radio" role="radiogroup" aria-label="Compactar a conversa a partir de">
        {COMPACT_OPTIONS.map((o) => (
          <label key={o.value} className="set-radio-it">
            <input type="radio" name="autocompact" checked={limit === o.value} disabled={limit == null} onChange={() => choose(o.value)} />
            {o.label}
          </label>
        ))}
        {limit != null && !COMPACT_OPTIONS.some((o) => o.value === limit) && <span className="small">Atual: {limit} tokens</span>}
      </div>
    </PageBody>
  );
}

function StartupPage() {
  const [info, setInfo] = useState<AutostartInfo | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    api.autostartGet().then(setInfo, (e) => setErr(errorMessage(e)));
  }, []);
  const toggle = (on: boolean) => {
    setBusy(true);
    setErr(null);
    api
      .autostartSet(on)
      .then(setInfo, (e) => setErr(errorMessage(e)))
      .finally(() => setBusy(false));
  };
  return (
    <PageBody
      title="Inicialização"
      lead="As tarefas agendadas só rodam com o Argus aberto. Para não depender de lembrar de abri-lo, ele pode iniciar junto com o Windows, minimizado."
    >
      {err && <div className="err" role="alert">{err}</div>}
      <label className="sa-check">
        <input type="checkbox" checked={info?.enabled ?? false} disabled={!info?.available || busy} onChange={(e) => toggle(e.target.checked)} />
        Iniciar com o Windows
      </label>
      {info && !info.available && info.reason && <p className="ui-sm ui-mut">{info.reason}</p>}
      <p className="ui-sm ui-mut sa-lead">Vale só para o seu usuário do Windows e não pede permissão de administrador.</p>
    </PageBody>
  );
}

function ShortcutsPage() {
  return (
    <PageBody title="Atalhos" lead="Atalhos de teclado do Argus. Ainda não dá para trocar as teclas.">
      <table className="ui-t sa-narrow">
        <thead>
          <tr>
            <th style={{ width: 140 }}>Tecla</th>
            <th>Ação</th>
          </tr>
        </thead>
        <tbody>
          {SHORTCUTS.map(([k, d]) => (
            <tr key={k}>
              <td>
                <span className="kbd">{k}</span>
              </td>
              <td>{d}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </PageBody>
  );
}

function McpPage() {
  const [on, setOn] = useState<boolean | null>(null);
  const [err, setErr] = useState<string | null>(null);
  useEffect(() => {
    api.devicesStatus().then((s) => setOn(s.agentsMcp), (e) => setErr(String(e)));
  }, []);
  const toggle = (v: boolean) =>
    api.devicesSetAgents(v).then((s) => setOn(s.agentsMcp), (e) => setErr(String(e)));
  return (
    <PageBody title="Ferramentas MCP" lead="Servidores MCP que o Argus oferece às conversas de agente.">
      {err && <div className="err small">{err}</div>}
      <table className="ui-t sa-narrow">
        <thead>
          <tr>
            <th>Servidor</th>
            <th>Descrição</th>
            <th style={{ width: 120 }}>Nas conversas</th>
          </tr>
        </thead>
        <tbody>
          <tr>
            <td className="ui-mono">screen-mirror</td>
            <td className="ui-mut">Controle do celular Android por ADB (toques, telas, logcat).</td>
            <td>
              <div className="ui-seg" role="radiogroup" aria-label="MCP do Screen-Mirror">
                <button type="button" role="radio" aria-checked={on === true} className={on ? "on-ok" : undefined} onClick={() => void toggle(true)}>
                  Ligado
                </button>
                <button type="button" role="radio" aria-checked={on === false} className={on === false ? "on" : undefined} onClick={() => void toggle(false)}>
                  Desligado
                </button>
              </div>
            </td>
          </tr>
        </tbody>
      </table>
    </PageBody>
  );
}

function SoonPage({ title, lead, text }: { title: string; lead: string; text: string }) {
  return (
    <PageBody title={title} lead={lead}>
      <div className="ui-empty sa-soon">
        <Chip>em breve</Chip>
        <span>{text}</span>
      </div>
    </PageBody>
  );
}

function AutoworkPage() {
  const projects = useApp((s) => s.projects);
  const selected = useApp((s) => s.selected);
  const [chosen, setChosen] = useState<Path | null>(null);
  const project = projects.find((p) => samePath(p.path, chosen ?? selected)) ?? projects[0] ?? null;
  return (
    <PageBody title="Autowork" lead="Trabalho automático por projeto.">
      {project ? (
        <>
          <label className="set-field sa-narrow">
            <span className="small">Projeto</span>
            <Select ariaLabel="Projeto" value={project.path} options={projects.map((p) => ({ value: p.path, label: p.name }))} onChange={setChosen} />
          </label>
          <div className="ui-card sa-narrow">
            <AutoWorkPanel key={project.path} project={project.path} />
          </div>
        </>
      ) : (
        <p className="ui-empty">Abra uma pasta raiz para configurar os projetos.</p>
      )}
    </PageBody>
  );
}

export default function SettingsArea() {
  const section = useWorkbench((s) => s.settingsSection);
  const selected = useApp((s) => s.selected);
  const [page, setPage] = useState<Page>(FROM_SECTION[section] ?? "aparencia");
  useEffect(() => {
    setPage(FROM_SECTION[section] ?? "aparencia");
  }, [section]);
  const [query, setQuery] = useState("");
  const search = useRef<HTMLInputElement>(null);
  const { meta, setRemote } = useSideStatus(selected);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const t = e.target as HTMLElement | null;
      if (e.key !== "/" || e.ctrlKey || e.altKey || e.metaKey) return;
      if (t && (t.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(t.tagName))) return;
      if (!search.current?.offsetParent) return; // área escondida
      e.preventDefault();
      search.current.focus();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const groups = useMemo(() => {
    const q = fold(query.trim());
    if (!q) return GROUPS;
    return GROUPS.map((g) => ({ ...g, items: g.items.filter((i) => fold(`${g.label} ${i.label} ${i.keywords}`).includes(q)) })).filter(
      (g) => g.items.length,
    );
  }, [query]);

  const label = ALL.find((i) => i.id === page)?.label ?? "";

  return (
    <div className="ui-area sa">
      <SideList width={220} label="Seções de Configurações">
        <div className="sa-search">
          <input
            ref={search}
            type="search"
            placeholder="Buscar configuração…"
            aria-label="Buscar configuração"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Escape") setQuery("");
              if (e.key === "Enter" && groups[0]?.items[0]) setPage(groups[0].items[0].id);
            }}
          />
          <span className="kbd">/</span>
        </div>
        {groups.map((g) => (
          <div key={g.label}>
            <SideHeading>{g.label}</SideHeading>
            {g.items.map((i) => (
              <SideItem key={i.id} on={page === i.id} onClick={() => setPage(i.id)} meta={meta[i.id]}>
                {i.label}
              </SideItem>
            ))}
          </div>
        ))}
        {!groups.length && <p className="ui-empty ui-sm">Nada encontrado.</p>}
      </SideList>
      {page === "permissoes" ? (
        <PermissionsPage />
      ) : (
        <div className="ui-area-main sa-main" aria-label={label}>
          {page === "aparencia" && <ThemePage />}
          {page === "atalhos" && <ShortcutsPage />}
          {page === "provedores" && (
            <PageBody title="Provedores" lead="As CLIs de agente instaladas e o login de cada uma.">
              <ProvidersView />
            </PageBody>
          )}
          {page === "subagentes" && <AgentDefsPage />}
          {page === "contexto" && <ContextPage />}
          {page === "mcp" && <McpPage />}
          {page === "inicializacao" && <StartupPage />}
          {page === "deps" && (
            <PageBody title="Dependências" lead="Ferramentas que faltam no projeto, cada uma com o comando de correção.">
              <DepsView />
            </PageBody>
          )}
          {page === "auxiliares" && (
            <SoonPage
              title="Auxiliares"
              lead="Processos de apoio do Argus, como o helper elevado para firewall e serviços."
              text="O Argus ainda não informa o estado dos auxiliares."
            />
          )}
          {page === "remoto" && (
            <PageBody title="Acesso remoto" lead="Use o Argus pelo celular na mesma rede.">
              <RemoteView onRunningChange={setRemote} />
            </PageBody>
          )}
          {page === "auditoria" && (
            <SoonPage title="Auditoria" lead="Decisões de permissão tomadas pelos agentes." text="O registro de auditoria (audit.log) ainda não existe no backend." />
          )}
          {page === "pet" && (
            <PageBody title="Pet" lead="O mascote da barra de status: o humor dele vem do uso do Argus.">
              <PetPage />
            </PageBody>
          )}
          {page === "player" && (
            <PageBody title="Player" lead="Controle a música sem sair do Argus: a mídia do Windows ou uma pasta de arquivos.">
              <PlayerPage />
            </PageBody>
          )}
          {page === "autowork" && <AutoworkPage />}
          <AreaStatus>salvo automaticamente</AreaStatus>
        </div>
      )}
    </div>
  );
}
