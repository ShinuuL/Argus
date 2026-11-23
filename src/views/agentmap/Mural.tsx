/**
 * "Arquivos em uso" no painel Agentes: o mural, visto de fora.
 *
 * O backend anota sozinho cada ferramenta de arquivo que passa pelos agentes
 * (crates/core/src/mural.rs) e avisa os outros no começo do turno seguinte.
 * Aqui o usuário vê a mesma coisa — útil sobretudo para flagrar, antes do diff,
 * duas sessões no mesmo arquivo.
 */
import { useEffect } from "react";
import { type MuralClaim, type TaskId } from "@/api";
import { Icon } from "@/shell/icons";
import { useMural } from "@/store";

/** Só o nome do arquivo: o caminho inteiro não cabe no painel e vai no `title`. */
const nome = (p: string) => p.replace(/\\/g, "/").split("/").pop() || p;

/** "agora", "há 3 min": a hora exata não interessa, a ordem de grandeza sim. */
function quando(at: number): string {
  const s = Math.max(0, Math.round(Date.now() / 1000 - at));
  if (s < 60) return "agora";
  return `há ${Math.floor(s / 60)} min`;
}

function Linha({ c, conflito }: { c: MuralClaim; conflito: boolean }) {
  return (
    <li className={`ap-mural-item${conflito ? " conflito" : ""}`}>
      <Icon name={c.write ? "pencil" : "eye"} size={12} />
      <span className="ap-mural-nome" title={c.path}>
        {nome(c.path)}
      </span>
      <span className="ap-mural-quem">
        #{c.task}
        {c.agent ? ` · ${c.agent}` : ""}
      </span>
      <span className="ap-mural-quando">{quando(c.at)}</span>
    </li>
  );
}

export function Mural({ taskId }: { taskId: TaskId }) {
  const claims = useMural((s) => s.claims);
  const load = useMural((s) => s.load);

  // Retrato inicial: quem abre o painel no meio de um turno não espera o
  // próximo evento para ver o que já está em andamento.
  useEffect(() => void load(), [load]);

  if (claims.length === 0) return null;

  // Arquivo em que mais de uma tarefa encostou: é o que o painel existe para mostrar.
  const porArquivo = new Map<string, Set<TaskId>>();
  for (const c of claims) {
    const k = c.path.replace(/\\/g, "/").toLowerCase();
    porArquivo.set(k, (porArquivo.get(k) ?? new Set()).add(c.task));
  }
  const disputado = (c: MuralClaim) => (porArquivo.get(c.path.replace(/\\/g, "/").toLowerCase())?.size ?? 1) > 1;
  const conflitos = claims.filter(disputado).length;
  const desta = claims.filter((c) => c.task === taskId).length;

  return (
    <section className="ap-mural" aria-label="Arquivos em uso">
      <header className="ap-head">
        <div className="ap-head-t">
          <h3>Arquivos em uso</h3>
          <span className={conflitos > 0 ? "ap-sub alerta" : "ap-sub"}>
            {conflitos > 0
              ? `${conflitos} arquivo(s) com mais de uma sessão dentro`
              : `${desta} desta sessão, ${claims.length - desta} de outras`}
          </span>
        </div>
      </header>
      <ul className="ap-mural-list" role="list">
        {claims.slice(0, 12).map((c) => (
          <Linha key={`${c.task}|${c.agent ?? ""}|${c.path}`} c={c} conflito={disputado(c)} />
        ))}
      </ul>
    </section>
  );
}
