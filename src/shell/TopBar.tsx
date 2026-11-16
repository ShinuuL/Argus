/**
 * Barra de título própria (a janela não tem decorações do sistema): não há uma
 * faixa separada. As abas do workbench ficam na linha de cima, e estes dois
 * grupos de botões flutuam nos cantos dessa linha:
 *
 *   [alternar barra esquerda] … abas … [terminal] [painel direito] [— □ ✕]
 *
 * O arrasto da janela vem dos espaços vazios marcados com
 * `data-tauri-drag-region` (título da barra esquerda, sobra da faixa de abas e
 * topo do painel direito); o duplo clique neles maximiza ou restaura.
 */
import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useLayout } from "@/store";
import { Icon } from "./icons";

/** A janela do Tauri, ou null fora dele (ex.: navegador comum no `vite`). */
function appWindow() {
  try {
    return getCurrentWindow();
  } catch {
    return null;
  }
}

function useMaximized(): boolean {
  const [max, setMax] = useState(false);
  useEffect(() => {
    const w = appWindow();
    if (!w) return;
    let alive = true;
    const sync = () => void w.isMaximized().then((m) => alive && setMax(m), () => {});
    sync();
    const off = w.onResized(sync);
    return () => {
      alive = false;
      void off.then((f) => f(), () => {});
    };
  }, []);
  return max;
}

/** Canto esquerdo: alternar a barra de projetos. */
export function TitleLead() {
  const leftOpen = useLayout((s) => s.leftOpen);
  return (
    <div className="tb-lead">
      <button
        className="tb-btn"
        aria-pressed={leftOpen}
        title={leftOpen ? "Ocultar a barra de projetos (Ctrl+B)" : "Mostrar a barra de projetos (Ctrl+B)"}
        aria-label="Alternar a barra de projetos"
        onClick={() => useLayout.getState().toggleLeft()}
      >
        <Icon name="sidebarLeft" size={16} />
      </button>
    </div>
  );
}

/** Canto direito: terminal, painel direito e os botões da janela. */
export function TitleControls() {
  const rightOpen = useLayout((s) => s.rightOpen);
  const bottomOpen = useLayout((s) => s.bottomOpen);
  const max = useMaximized();
  const layout = useLayout.getState();
  const win = (fn: (w: NonNullable<ReturnType<typeof appWindow>>) => Promise<void>) => () => {
    const w = appWindow();
    if (w) void fn(w).catch(() => {});
  };

  return (
    <div className="tb-ctl">
      <button
        className="tb-btn"
        aria-pressed={bottomOpen}
        title={bottomOpen ? "Ocultar o terminal (Ctrl+J)" : "Mostrar o terminal (Ctrl+J)"}
        aria-label="Alternar o terminal"
        onClick={layout.toggleBottom}
      >
        <Icon name="terminal" size={16} />
      </button>
      <button
        className="tb-btn"
        aria-pressed={rightOpen}
        title={rightOpen ? "Ocultar o painel direito" : "Mostrar o painel direito (arquivos, alterações, subagentes, servidores)"}
        aria-label="Alternar o painel direito"
        onClick={layout.toggleRight}
      >
        <Icon name="sidebarRight" size={16} />
      </button>
      <div className="tb-win" role="group" aria-label="Janela">
        <button className="tb-wbtn" title="Minimizar" aria-label="Minimizar" onClick={win((w) => w.minimize())}>
          <Icon name="minus" size={16} />
        </button>
        <button
          className="tb-wbtn"
          title={max ? "Restaurar" : "Maximizar"}
          aria-label={max ? "Restaurar" : "Maximizar"}
          onClick={win((w) => w.toggleMaximize())}
        >
          <Icon name={max ? "restore" : "maximize"} size={13} />
        </button>
        <button className="tb-wbtn close" title="Fechar" aria-label="Fechar" onClick={win((w) => w.close())}>
          <Icon name="close" size={16} />
        </button>
      </div>
    </div>
  );
}
