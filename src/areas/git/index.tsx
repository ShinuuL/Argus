/**
 * Área "Git" em tela cheia (telas 05 e 10 do mockup): abas Pull requests,
 * Repositórios, Branches locais e Alterações (revisão da sessão em foco).
 * Os dados vêm do `gh` (connections_repo), do status git dos projetos e do
 * diff da sessão. O que o backend ainda não fornece (checks, revisões, data
 * de atualização dos PRs, lista de branches) aparece como "—" ou estado vazio.
 */
import { useCallback, useEffect, useMemo, useState } from "react";
import { api, baseName, errorMessage, type Path } from "@/api";
import { useApp, useLayout, useWorkbench } from "@/store";
import { useActiveContext } from "@/shell/useActiveContext";
import { connectionsRepo, connectionsStatus, type Connection, type RepoInfo } from "@/views/connections/api";
import { AreaHeader, Btn, Chip, DataTable, Empty, StatusDot, Tab, Tabs, type Column, AreaStatus } from "@/ui";
import { Icon } from "@/shell/icons";
import { Review, useSessionDiff } from "./Review";
import "./git.css";

type GitTab = "prs" | "repos" | "branches" | "changes";

interface PrRow {
  key: string;
  project: Path;
  repo: RepoInfo;
  number: number;
  title: string;
}

const NO_DATA = "O Argus ainda não lê esta informação do GitHub";

/** Repositório GitHub de cada projeto (undefined = consultando, null = não é GitHub, string = erro). */
function useRepos(projects: Path[], enabled: boolean) {
  const [repos, setRepos] = useState<Record<Path, RepoInfo | null | string | undefined>>({});
  const [rev, setRev] = useState(0);
  const key = projects.join("|");
  useEffect(() => {
    if (!enabled) return;
    let alive = true;
    setRepos(Object.fromEntries(projects.map((p) => [p, undefined])));
    (async () => {
      // Um por vez: cada consulta chama o `gh`, e várias em paralelo travam a rede.
      for (const p of projects) {
        const r = await connectionsRepo(p).catch((e) => errorMessage(e));
        if (!alive) return;
        setRepos((s) => ({ ...s, [p]: r }));
      }
    })();
    return () => {
      alive = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key, enabled, rev]);
  return { repos, refresh: () => setRev((n) => n + 1) };
}

function useConnections() {
  const [list, setList] = useState<Connection[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const refresh = useCallback(() => {
    setList(null);
    setError(null);
    connectionsStatus().then(setList, (e) => setError(errorMessage(e)));
  }, []);
  useEffect(refresh, [refresh]);
  return { list, error, refresh };
}

export default function GitArea() {
  const projects = useApp((s) => s.projects);
  const statuses = useApp((s) => s.statuses);
  const { taskId, project } = useActiveContext();
  const [tab, setTab] = useState<GitTab>(taskId != null ? "changes" : "prs");
  const [selPr, setSelPr] = useState<string | null>(null);
  // showChanges() (comando /diff, barra de status, "Revisar" do inspetor) sempre cai na revisão.
  const diffRev = useLayout((s) => s.diffRev);
  useEffect(() => {
    if (diffRev > 0) setTab("changes");
  }, [diffRev]);
  const conns = useConnections();
  const gitProjects = useMemo(() => projects.map((p) => p.path).filter((p) => statuses[p] != null), [projects, statuses]);
  const { repos, refresh } = useRepos(gitProjects, tab === "prs" || tab === "repos");
  const { files } = useSessionDiff(taskId);

  const gh = conns.list?.find((c) => c.id === "gh");
  const prs: PrRow[] = useMemo(
    () =>
      gitProjects.flatMap((p) => {
        const r = repos[p];
        return r && typeof r === "object"
          ? r.pull_requests.map((pr) => ({ key: `${r.name_with_owner}#${pr.number}`, project: p, repo: r, number: pr.number, title: pr.title }))
          : [];
      }),
    [gitProjects, repos],
  );
  const loading = gitProjects.some((p) => repos[p] === undefined);
  const ghRepos = gitProjects.filter((p) => repos[p] && typeof repos[p] === "object").length;
  const pr = prs.find((r) => r.key === selPr) ?? null;
  const changesCount = taskId != null ? files.length : (project ? statuses[project]?.changed_files.length : 0) ?? 0;

  const prCols: Column<PrRow>[] = [
    { key: "pr", header: "PR", render: (r) => `#${r.number} ${r.title}` },
    { key: "repo", header: "Repositório", render: (r) => r.repo.name_with_owner.split("/").pop() },
    { key: "branch", header: "Branch", render: () => <span className="ui-mut" title={NO_DATA}>—</span> },
    { key: "checks", header: "Checks", render: () => <Chip title={NO_DATA}>—</Chip> },
    { key: "rev", header: "Revisão", render: () => <Chip title={NO_DATA}>—</Chip> },
    { key: "upd", header: "Atualizado", render: () => <span className="ui-mut" title={NO_DATA}>—</span> },
  ];

  const repoCols: Column<Path>[] = [
    { key: "name", header: "Projeto", render: (p) => baseName(p) },
    {
      key: "gh",
      header: "GitHub",
      render: (p) => {
        const r = repos[p];
        return r === undefined ? <span className="ui-mut">consultando…</span> : r === null ? <span className="ui-mut">não é GitHub</span> : typeof r === "string" ? <Chip tone="bad" title={r}>erro</Chip> : r.name_with_owner;
      },
    },
    { key: "branch", header: "Branch", render: (p) => <span className="ui-mono">{statuses[p]?.branch ?? "destacado"}</span> },
    { key: "def", header: "Padrão", render: (p) => { const r = repos[p]; return <span className="ui-mono ui-mut">{r && typeof r === "object" ? (r.default_branch ?? "—") : "—"}</span>; } },
    { key: "chg", header: "Alterados", right: true, render: (p) => statuses[p]?.changed_files.length ?? 0 },
    { key: "prs", header: "PRs", right: true, render: (p) => { const r = repos[p]; return r && typeof r === "object" ? r.pull_requests.length : "—"; } },
    {
      key: "open",
      header: "",
      right: true,
      render: (p) => {
        const r = repos[p];
        return r && typeof r === "object" ? (
          <Btn variant="ghost" small onClick={(e) => (e.stopPropagation(), void api.systemOpenUrl(r.url))}>Abrir</Btn>
        ) : null;
      },
    },
  ];

  const branchCols: Column<Path>[] = [
    { key: "name", header: "Projeto", render: (p) => baseName(p) },
    { key: "branch", header: "Branch atual", render: (p) => <span className="ui-mono">{statuses[p]?.branch ?? "HEAD destacado"}</span> },
    {
      key: "sync",
      header: "Remoto",
      render: (p) => {
        const s = statuses[p];
        if (!s) return "—";
        if (!s.ahead && !s.behind) return <Chip tone="ok">em dia</Chip>;
        return (
          <span className="gt-chips">
            {s.ahead > 0 && <Chip tone="info">{s.ahead} à frente</Chip>}
            {s.behind > 0 && <Chip tone="warn">{s.behind} atrás</Chip>}
          </span>
        );
      },
    },
    { key: "last", header: "Último commit", render: (p) => <span className="ellipsis gt-last">{statuses[p]?.last_commit?.message.split("\n")[0] ?? "—"}</span> },
    { key: "when", header: "Quando", right: true, render: (p) => { const c = statuses[p]?.last_commit; return c ? new Date(c.time * 1000).toLocaleString("pt-BR", { dateStyle: "short", timeStyle: "short" }) : "—"; } },
  ];

  const askAgent = (r: PrRow) => {
    const text = `Resolva os comentários de revisão do PR #${r.number} (${r.title}) em ${r.repo.url}/pull/${r.number}. Use \`gh pr view ${r.number} --comments\` para ler a revisão.`;
    void navigator.clipboard?.writeText(text).catch(() => {});
    useWorkbench.getState().openNewChat(r.project);
  };

  /*
   * Sem projeto com git, as quatro abas estão vazias **pelo mesmo motivo** —
   * então o motivo é dito uma vez, no lugar delas, em vez de quatro vezes. E
   * com a saída junto: antes a tela diagnosticava e parava, com "Abrir pasta
   * raiz" a uma tela de distância.
   */
  const semGit = !loading && gitProjects.length === 0 && tab !== "changes";

  return (
    <div className="ui-area">
      <div className="ui-area-main gt-area">
        {semGit ? (
          <>
            <AreaHeader title="Git" sub="nenhum repositório" actions={<Btn variant="ghost" small disabled>Atualizar</Btn>} />
            <Empty
              icon={<Icon name="diff" size={20} />}
              title="Nenhum projeto com git na pasta raiz"
              action={<Btn onClick={() => void useApp.getState().pickRoot()}>Abrir pasta raiz</Btn>}
            >
              Com a pasta aberta, aqui ficam os pull requests, os repositórios, as branches locais e
              as alterações que o agente fez — prontas para revisar antes de virar commit.
            </Empty>
          </>
        ) : (
        <>
        {tab !== "changes" && (
          <AreaHeader
            title="Git"
            sub={`${gitProjects.length} repositório${gitProjects.length === 1 ? "" : "s"}${prs.length ? ` · ${prs.length} PR${prs.length === 1 ? "" : "s"} aberto${prs.length === 1 ? "" : "s"}` : ""}`}
          />
        )}
        <div className="gt-top">
          <Tabs label="Seções do Git" className="gt-tabs">
            <Tab on={tab === "prs"} onClick={() => setTab("prs")} count={loading ? undefined : prs.length}>Pull requests</Tab>
            <Tab on={tab === "repos"} onClick={() => setTab("repos")} count={gitProjects.length}>Repositórios</Tab>
            <Tab on={tab === "branches"} onClick={() => setTab("branches")}>Branches locais</Tab>
            <Tab on={tab === "changes"} onClick={() => setTab("changes")} count={changesCount || undefined}>Alterações</Tab>
          </Tabs>
          <span className="ui-sp" />
          {(tab === "prs" || tab === "repos") && (
            <Btn variant="ghost" small onClick={() => (refresh(), conns.refresh())} disabled={loading}>
              {loading ? "Consultando…" : "Atualizar"}
            </Btn>
          )}
        </div>

        {tab === "changes" ? (
          <Review taskId={taskId} project={project} />
        ) : (
          <div className="gt-body">
            {tab === "prs" && (
              <>
                <DataTable
                  columns={prCols}
                  rows={prs}
                  rowKey={(r) => r.key}
                  selected={selPr}
                  onRowClick={(r) => setSelPr(r.key)}
                  empty={
                    <Empty
                      icon={<Icon name="diff" size={20} />}
                      title={loading ? "Consultando o GitHub…" : "Nenhum PR aberto"}
                    >
                      {loading ? "Lendo os repositórios pela CLI do gh." : "Nenhum dos repositórios do GitHub tem pull request aberto agora."}
                    </Empty>
                  }
                />
                {pr && (
                  <div className="ui-card gt-action">
                    <StatusDot tone="info" />
                    <span className="gt-grow">
                      #{pr.number} {pr.title} · <span className="ui-mut">{pr.repo.name_with_owner}</span>
                    </span>
                    <Btn variant="ghost" onClick={() => void api.systemOpenUrl(`${pr.repo.url}/pull/${pr.number}`)}>Abrir no navegador</Btn>
                    <Btn onClick={() => askAgent(pr)} title="Abre uma conversa nova no projeto; o pedido vai para a área de transferência">
                      Pedir ao agente para resolver
                    </Btn>
                  </div>
                )}
              </>
            )}
            {tab === "repos" && (
              <>
                <DataTable columns={repoCols} rows={gitProjects} rowKey={(p) => p} />
                <div className="ui-card">
                  <div className="ui-card-h">Login das CLIs</div>
                  {conns.error && <p className="ui-mut ui-sm">{conns.error}</p>}
                  {conns.list === null && !conns.error && <p className="ui-mut ui-sm">Consultando as CLIs…</p>}
                  <div className="gt-conns">
                    {conns.list?.map((c) => (
                      <div key={c.id} className="gt-conn">
                        <StatusDot tone={c.logged_in ? "ok" : c.installed ? "warn" : undefined} />
                        <b>{c.label}</b>
                        <span className="ui-mut ui-sm gt-grow">
                          {!c.installed ? "não instalada" : c.error ? `não verificado (${c.error})` : c.logged_in ? `conectado${c.account ? ` como ${c.account}` : ""}` : "não conectado"}
                        </span>
                        {c.installed && !c.logged_in && (
                          <Btn small variant="ghost" onClick={() => useWorkbench.getState().openTerminal({ title: `login ${c.id}`, command: c.login })}>
                            Fazer login
                          </Btn>
                        )}
                      </div>
                    ))}
                  </div>
                </div>
              </>
            )}
            {tab === "branches" && (
              <>
                <DataTable columns={branchCols} rows={gitProjects} rowKey={(p) => p} />
                <p className="ui-mut ui-sm">Mostra a branch atual de cada projeto. A lista completa de branches locais ainda não tem comando no Argus.</p>
              </>
            )}
          </div>
        )}
        </>
        )}

        {tab !== "changes" && (
          <AreaStatus>
            <StatusDot tone={gh?.logged_in ? "ok" : gh ? "warn" : undefined} />
            <span>{!gh ? "GitHub: verificando…" : gh.logged_in ? `GitHub conectado${gh.account ? ` · ${gh.account}` : ""}` : gh.installed ? "GitHub não conectado" : "CLI gh não instalada"}</span>
            <span className="ui-sp" />
            <span>{ghRepos} de {gitProjects.length} repositórios no GitHub</span>
          </AreaStatus>
        )}
      </div>
    </div>
  );
}
