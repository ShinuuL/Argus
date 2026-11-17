import { useEffect, useState } from "react";
import { api, errorMessage, type Path } from "@/api";
import { connectionsRepo, type RepoInfo } from "./api";

/** Cartão do repositório GitHub de um projeto: nome, branch padrão e até 5 PRs abertos. */
export function RepoCard({ project }: { project: Path }) {
  const [info, setInfo] = useState<RepoInfo | null | undefined>(undefined);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let alive = true;
    setInfo(undefined);
    setError(null);
    connectionsRepo(project)
      .then((r) => alive && setInfo(r))
      .catch((e) => alive && setError(errorMessage(e)));
    return () => {
      alive = false;
    };
  }, [project]);

  if (error) return <div className="conn-card small muted">{error}</div>;
  if (info === undefined) return <div className="conn-card small muted">Consultando o GitHub…</div>;
  if (info === null) return <div className="conn-card small muted">Este projeto não é um repositório do GitHub.</div>;

  return (
    <div className="conn-card">
      <div className="conn-row">
        <b>{info.name_with_owner}</b>
        {info.default_branch && <span className="small muted">branch padrão: {info.default_branch}</span>}
        <button onClick={() => void api.systemOpenUrl(info.url)}>Abrir no GitHub</button>
      </div>
      {info.pull_requests.length === 0 ? (
        <span className="small muted">Nenhum PR aberto.</span>
      ) : (
        <ul className="conn-prs">
          {info.pull_requests.map((pr) => (
            <li key={pr.number}>
              <a href="#" onClick={(e) => (e.preventDefault(), void api.systemOpenUrl(`${info.url}/pull/${pr.number}`))}>
                #{pr.number}
              </a>{" "}
              {pr.title}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
