import { useRef } from "react";
import { isTaskActive } from "@/api";
import { useApp, useLayout, useWorkbench } from "@/store";
import { useActiveContext } from "./useActiveContext";
import { BuildsStatusButton } from "@/views/builds";
import PetView from "@/views/pet";
import PlayerView from "@/views/player";
import { ServersStatusButton } from "@/views/servers";
import { sessionTitle } from "@/views/session/events";
import { Icon } from "./icons";
import { AREA_STATUS_SLOT } from "@/ui/AreaStatus";

/** "N agentes rodando": clicar foca a sessão (várias: cada clique vai para a próxima). */
function AgentsRunning() {
  const tasks = useApp((s) => s.tasks);
  const running = tasks.filter((t) => isTaskActive(t.state));
  const next = useRef(0);
  const n = running.length;
  const label = n === 0 ? "Nenhum agente rodando" : n === 1 ? "1 agente rodando" : `${n} agentes rodando`;
  const focus = () => {
    if (n === 0) return;
    const task = running[next.current % n];
    next.current += 1;
    useWorkbench.getState().openSession(task.id);
  };
  return (
    <button
      className={n > 0 ? "wb-sbtn sb-item sb-running" : "wb-sbtn sb-item"}
      disabled={n === 0}
      title={n === 0 ? label : `${running.map((t) => `#${t.id} ${sessionTitle(t.prompt, 50)}`).join("\n")}\n\nClique para abrir a sessão`}
      onClick={focus}
    >
      {n > 0 ? <span className="ses-dot running" aria-hidden="true" /> : <Icon name="bot" size={13} />}
      {label}
    </button>
  );
}

/** Contexto da conversa em foco (mockup .sb): agente, branch e alterações do projeto. */
function FocusInfo() {
  const ctx = useActiveContext();
  const task = useApp((s) => (ctx.taskId == null ? undefined : s.tasks.find((t) => t.id === ctx.taskId)));
  const status = useApp((s) => (ctx.project ? s.statuses[ctx.project] : undefined));
  const changed = status?.changed_files.length ?? 0;
  return (
    <>
      {task && <span className="sb-item">● {task.provider}</span>}
      {status?.branch && <span className="sb-item">{status.branch}</span>}
      {changed > 0 && (
        <button className="wb-sbtn sb-item" title="Ver as alterações no inspetor" onClick={() => useLayout.getState().showChanges()}>
          {changed === 1 ? "1 alteração" : `${changed} alterações`}
        </button>
      )}
    </>
  );
}

/** Atalhos à direita, como no mockup; com o inspetor aberto, só o aviso. */
function Hints() {
  const area = useLayout((s) => s.area);
  const inspector = useLayout((s) => s.inspectorOpen);
  if (area !== "chat") return null;
  return <span className="sb-item sb-hint">{inspector ? "inspetor aberto" : "Ctrl+J terminal · Ctrl+. inspetor"}</span>;
}

/** Barra de status: agentes rodando, servidores e uso de hoje à esquerda; player, divisão e pet à direita. */
export function StatusBar() {
  const split = useWorkbench((s) => s.split);
  const toggleSplitDir = useWorkbench((s) => s.toggleSplitDir);
  const area = useLayout((s) => s.area);

  return (
    <footer className="wb-status">
      <div className="wb-status-l">
        {area === "chat" || area === "terminal" ? <FocusInfo /> : null}
        <div id={AREA_STATUS_SLOT} className="sb-area" />
        <AgentsRunning />
        <ServersStatusButton />
        <BuildsStatusButton />
      </div>
      <div className="wb-status-r">
        <Hints />
        <PlayerView />
        <button
          className="wb-sbtn sb-item"
          title={split === "horizontal" ? "Grupos lado a lado (clique para empilhar)" : "Grupos empilhados (clique para pôr lado a lado)"}
          aria-label="Alternar a direção da divisão"
          onClick={toggleSplitDir}
        >
          <Icon name={split === "horizontal" ? "splitH" : "splitV"} size={13} />
        </button>
        <PetView />
      </div>
    </footer>
  );
}
