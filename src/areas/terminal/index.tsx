/**
 * Tela 09 · Terminais (docs/mockups/argus-telas.html, #s9).
 *
 * Os terminais (src/views/terminal) precisam ficar sempre montados para os
 * processos continuarem vivos, então o Shell não desmonta o painel inferior:
 * nesta área ele esconde o resto da conversa e expande o painel inferior
 * (`.sh-chat.term-full`). Este componente entra antes de `.sh-main` com
 * `display: contents`; o terminal-area.css transforma `.sh-chat.term-full` numa
 * grade (cabeçalho em cima, lista de terminais e builds à esquerda, terminais à
 * direita). "Dividir" mostra todos os terminais abertos lado a lado.
 */
import { useEffect, useMemo, useState } from "react";
import { defaultCwd, useLayout, useTerminals } from "@/store";
import { useRuns } from "@/views/builds";
import { formatDuration, isActive, STATE_LABEL } from "@/views/builds/runs";
import { baseName, type RunInfo } from "@/api";
import { AreaHeader, Btn, SideHeading, SideItem, SideList, StatusDot } from "@/ui";
import "./terminal-area.css";

const SPLIT_ATTR = "termSplit";

function runTone(r: RunInfo): "ok" | "warn" | "bad" | undefined {
  if (isActive(r)) return "warn";
  if (r.state === "failed" || (r.exit_code != null && r.exit_code !== 0)) return "bad";
  if (r.state === "ok" || r.exit_code === 0) return "ok";
  return undefined;
}

export default function TerminalArea() {
  const tabs = useTerminals((s) => s.tabs);
  const active = useTerminals((s) => s.active);
  const activate = useTerminals((s) => s.activate);
  const openTab = useTerminals((s) => s.open);
  const runs = useRuns((s) => s.runs);
  const builds = useMemo(() => runs.filter((r) => r.kind === "build"), [runs]);
  const [split, setSplit] = useState(false);
  const canSplit = tabs.length > 1;
  const splitOn = split && canSplit;

  useEffect(() => {
    if (splitOn) document.body.dataset[SPLIT_ATTR] = "1";
    else delete document.body.dataset[SPLIT_ATTR];
    return () => {
      delete document.body.dataset[SPLIT_ATTR];
    };
  }, [splitOn]);

  const panes = splitOn ? tabs.length : Math.min(tabs.length, 1);

  return (
    <div className="ta-wrap">
      <div className="ta-head">
        <AreaHeader
          title="Terminais"
          sub={panes === 1 ? "1 painel" : `${panes} painéis`}
          actions={
            <>
              <Btn variant="ghost" small onClick={() => openTab({ cwd: defaultCwd() })}>
                Novo terminal
              </Btn>
              <Btn
                variant="ghost"
                small
                disabled={!canSplit}
                aria-pressed={splitOn}
                title={canSplit ? "Mostrar todos os terminais lado a lado" : "Abra dois terminais para dividir"}
                onClick={() => setSplit(!split)}
              >
                {splitOn ? "Um por vez" : "Dividir"}
              </Btn>
              <Btn variant="ghost" small onClick={() => useLayout.getState().setArea("chat")}>
                Voltar à conversa
              </Btn>
            </>
          }
        />
      </div>
      <div className="ta-side">
        <SideList width={200} label="Terminais e builds">
          <SideHeading>Abertos</SideHeading>
          {tabs.length === 0 && <div className="ta-empty ui-mut ui-sm">Nenhum terminal aberto.</div>}
          {tabs.map((t) => (
            <SideItem
              key={t.id}
              on={t.id === active}
              icon={<StatusDot tone="ok" />}
              title={t.cwd ?? t.title}
              onClick={() => activate(t.id)}
            >
              {t.cwd ? `${baseName(t.cwd)} · ${t.title}` : t.title}
            </SideItem>
          ))}
          <SideHeading>Builds</SideHeading>
          {builds.length === 0 && <div className="ta-empty ui-mut ui-sm">Nenhum build nesta execução do Argus.</div>}
          {builds.map((r) => (
            <SideItem
              key={r.id}
              icon={<StatusDot tone={runTone(r)} />}
              title={`${r.cmdline}\n${STATE_LABEL[r.state] ?? r.state}`}
              meta={r.ended_at ? formatDuration(r.ended_at - r.started_at) : STATE_LABEL[r.state] ?? r.state}
            >
              {baseName(r.project)} · {r.label}
            </SideItem>
          ))}
        </SideList>
      </div>
    </div>
  );
}
