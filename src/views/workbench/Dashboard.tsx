import { baseName } from "@/api";
import { useApp } from "@/store";

/** Painel com o status git de todos os projetos da pasta raiz. */
export function Dashboard() {
  const projects = useApp((s) => s.projects);
  const statuses = useApp((s) => s.statuses);
  const selected = useApp((s) => s.selected);
  const select = useApp((s) => s.select);

  if (projects.length === 0) {
    return <p className="muted pad">Escolha uma pasta raiz com seus projetos.</p>;
  }
  return (
    <div className="pad scroll">
      <table className="grid">
        <thead>
          <tr>
            <th>Projeto</th>
            <th>Branch</th>
            <th>Alterados</th>
            <th>↑ / ↓</th>
            <th>Último commit</th>
          </tr>
        </thead>
        <tbody>
          {projects.map((p) => {
            const st = statuses[p.path];
            const commit = st?.last_commit;
            return (
              <tr
                key={p.path}
                className={p.path === selected ? "dash-sel" : undefined}
                title={p.path}
                onClick={() => select(p.path)}
              >
                <td>{p.name || baseName(p.path)}</td>
                {st ? (
                  <>
                    <td>{st.branch ?? "(detached)"}</td>
                    <td className={st.changed_files.length ? "warn" : undefined}>{st.changed_files.length}</td>
                    <td>
                      {st.ahead} / {st.behind}
                    </td>
                    <td>{commit ? `${commit.id} ${commit.message.split("\n")[0]} (${commit.author})` : ""}</td>
                  </>
                ) : (
                  <td colSpan={4} className="muted">
                    {st === undefined ? "calculando…" : "sem repositório git"}
                  </td>
                )}
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}
