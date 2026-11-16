import { useRef } from "react";
import { isTaskActive } from "@/api";
import { useApp, useWorkbench } from "@/store";
import { UsagePill } from "@/views/chat";
import { BuildsStatusButton } from "@/views/builds";
import PetView from "@/views/pet";
import PlayerView from "@/views/player";
import { ServersStatusButton } from "@/views/servers";
import { sessionTitle } from "@/views/session/events";
import { Icon } from "./icons";

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

/** Barra de status: agentes rodando, servidores e uso de hoje à esquerda; player, divisão e pet à direita. */
export function StatusBar() {
  const split = useWorkbench((s) => s.split);
  const toggleSplitDir = useWorkbench((s) => s.toggleSplitDir);

  return (
    <footer className="wb-status">
      <div className="wb-status-l">
        <AgentsRunning />
        <ServersStatusButton />
        <BuildsStatusButton />
        <UsagePill />
      </div>
      <div className="wb-status-r">
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
