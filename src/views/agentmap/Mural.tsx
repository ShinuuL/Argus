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
const chavePath = (c: MuralClaim) => c.path.replace(/\\/g, "/").toLowerCase();

/** "agora", "há 3 min": a hora exata não interessa, a ordem de grandeza sim. */
function quando(at: number): string {
  const s = Math.max(0, Math.round(Date.now() / 1000 - at));
  if (s < 60) return "agora";
  return `há ${Math.floor(s / 60)} min`;
}

interface Grupo {
  task: TaskId;
  claims: MuralClaim[];
  /** Total antes do corte de linhas visíveis. */
  total: number;
}

function Linha({ c, conflito }: { c: MuralClaim; conflito: boolean }) {
  return (
    <li
      className={`ap-mural-item${c.write ? " write" : ""}${conflito ? " conflito" : ""}`}
      title={`${c.path} · ${c.write ? "escrita" : "leitura"} (${c.tool}) · ${quando(c.at)}`}
    >
      <Icon name={c.write ? "pencil" : "eye"} size={12} />
      <span className="ap-mural-nome">{nome(c.path)}</span>
      {c.agent && (
        <span className="ap-mural-agente" title={`Subagente ${c.agent}`}>
          {c.agent}
        </span>
      )}
      {conflito ? (
        <span className="ap-mural-alerta">
          <Icon name="alert" size={11} /> 2 sessões
        </span>
      ) : (
        <span className="ap-mural-quando">{quando(c.at)}</span>
      )}
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
    const k = chavePath(c);
    porArquivo.set(k, (porArquivo.get(k) ?? new Set()).add(c.task));
  }
  const disputado = (c: MuralClaim) => (porArquivo.get(chavePath(c))?.size ?? 1) > 1;
  const arquivosDisputados = [...porArquivo.values()].filter((ts) => ts.size > 1).length;
  const desta = claims.filter((c) => c.task === taskId).length;

  // Agrupar por sessão evita repetir "#36" em todas as linhas: o número passa a
  // aparecer uma vez, no cabeçalho do grupo. A sessão atual vem primeiro.
  const grupos = new Map<TaskId, MuralClaim[]>();
  for (const c of claims) grupos.set(c.task, [...(grupos.get(c.task) ?? []), c]);
  const ordenados = [...grupos.entries()].sort((a, b) =>
    a[0] === taskId ? -1 : b[0] === taskId ? 1 : b[1][0].at - a[1][0].at,
  );

  const LIMITE = 12;
  let usados = 0;
  let restantes = 0;
  const visiveis: Grupo[] = [];
  for (const [task, lista] of ordenados) {
    const n = Math.min(lista.length, Math.max(0, LIMITE - usados));
    if (n > 0) visiveis.push({ task, claims: lista.slice(0, n), total: lista.length });
    restantes += lista.length - n;
    usados += n;
  }

  const outras = claims.length - desta;
  const resumo =
    arquivosDisputados > 0
      ? `${arquivosDisputados} arquivo(s) em mais de uma sessão`
      : desta === 0
        ? `${outras} de outras sessões`
        : `${desta} desta sessão, ${outras} de outras`;

  return (
    <section className="ap-mural" aria-label="Arquivos em uso">
      <header className="ap-head">
        <div className="ap-head-t">
          <h3>Arquivos em uso</h3>
          <span className={arquivosDisputados > 0 ? "ap-sub alerta" : "ap-sub"}>{resumo}</span>
        </div>
      </header>
      <div className="ap-mural-grupos">
        {visiveis.map((g) => (
          <div className="ap-mural-grupo" key={g.task}>
            <div className="ap-mural-gh">
              <Icon name={g.task === taskId ? "circleDot" : "bot"} size={11} />
              <span>{g.task === taskId ? "Nesta sessão" : `Sessão #${g.task}`}</span>
              <span className="ap-mural-gn">{g.total}</span>
              <span className="ap-mural-gq">{quando(g.claims[0].at)}</span>
            </div>
            <ul className="ap-mural-list" role="list">
              {g.claims.map((c) => (
                <Linha key={`${c.task}|${c.agent ?? ""}|${c.path}`} c={c} conflito={disputado(c)} />
              ))}
            </ul>
          </div>
        ))}
      </div>
      {restantes > 0 && <div className="ap-mural-mais">+{restantes} arquivo(s) mais antigo(s)</div>}
    </section>
  );
}
