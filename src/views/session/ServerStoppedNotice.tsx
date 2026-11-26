/**
 * Um servidor de dev que o agente sobe pode cair no fim do turno (a CLI pode
 * encerrar os processos em segundo plano; no claude 2.1.220 ele sobreviveu). No fim do turno, se o último
 * pedido rodou um dos servidores detectados no projeto e, alguns segundos depois,
 * nenhum servidor está no ar na pasta do projeto, oferece iniciá-lo pelo Argus (terminal da IDE, que
 * continua depois do turno). Não custa token: nada vai no prompt.
 */
import { useEffect, useMemo, useState } from "react";
import { api, samePath, type AgentEvent, type DetectedCommand, type Path } from "@/api";
import { Icon } from "@/shell/icons";
import { commandsOf, serverOf, startCommand, useRuns } from "@/views/builds/runs";

/** Comandos de shell do último pedido (depois do último `User`). */
function lastTurnCommands(events: AgentEvent[]): string[] {
  let start = 0;
  events.forEach((e, i) => {
    if ("User" in e) start = i + 1;
  });
  const out: string[] = [];
  for (const e of events.slice(start)) {
    if (!("ToolUse" in e)) continue;
    const input = e.ToolUse.input as { command?: unknown } | null;
    if (typeof input?.command === "string") out.push(input.command.toLowerCase());
  }
  return out;
}

/** O comando de shell roda este servidor detectado? (`npm run dev`, `pnpm dev`, `cargo run`...) */
function runsServer(cmd: string, c: DetectedCommand): boolean {
  const program = c.program.toLowerCase().replace(/\.(cmd|exe|bat)$/, "");
  const args = c.args.join(" ").toLowerCase();
  return cmd.includes(program) && (args === "" || cmd.includes(args));
}

/** Algum servidor no ar dentro da pasta do projeto (do Argus, do agente ou de fora)? */
async function serverUpIn(project: Path): Promise<boolean> {
  const norm = (p: string) => p.replace(/[\\/]+/g, "/").replace(/\/$/, "").toLowerCase();
  const root = norm(project);
  const list = await api.serversList().catch(() => []);
  return list.some((s) => s.cwd != null && (samePath(s.cwd, project) || norm(s.cwd).startsWith(root + "/")));
}

export function ServerStoppedNotice({ events, project, busy }: { events: AgentEvent[]; project: Path | null; busy: boolean }) {
  const runs = useRuns((s) => s.runs);
  const commands = useRuns((s) => s.commands);
  // O cache de comandos só enche quando a área de servidores abre: detecta aqui se faltar.
  const [own, setOwn] = useState<DetectedCommand[]>([]);
  useEffect(() => {
    if (!project || busy) return;
    let alive = true;
    api.runsDetect(project).then((v) => alive && setOwn(v), () => undefined);
    return () => {
      alive = false;
    };
  }, [project, busy]);
  const [dismissed, setDismissed] = useState(-1);
  const [error, setError] = useState<string | null>(null);
  const server = useMemo(() => {
    if (busy || !project) return null;
    const known = commandsOf(commands, project);
    const servers = (known.length ? known : own).filter((c) => c.kind === "servidor");
    if (!servers.length) return null;
    const cmds = lastTurnCommands(events);
    return servers.find((s) => cmds.some((c) => runsServer(c, s))) ?? null;
  }, [busy, project, commands, own, events]);
  // Só depois de conferir que nenhum servidor ficou no ar (o claude 2.1 às vezes
  // mantém o processo): espera a CLI encerrar o que fosse encerrar.
  const [down, setDown] = useState(false);
  useEffect(() => {
    setDown(false);
    if (!server || !project) return;
    let alive = true;
    const t = setTimeout(() => {
      void serverUpIn(project).then((up) => alive && setDown(!up));
    }, 4000);
    return () => {
      alive = false;
      clearTimeout(t);
    };
  }, [server, project, events.length]);
  if (!server || !project || !down || dismissed === events.length || serverOf(runs, project)) return null;
  return (
    <div className="chat-notice warn" role="status">
      <pre>
        {error ??
          `O agente subiu "${server.label}" neste pedido, mas o servidor não está mais no ar. Pelo Argus ele continua rodando depois do turno.`}
      </pre>
      <button
        type="button"
        className="chat-notice-act"
        onClick={() =>
          startCommand(project, server.id).then(
            () => setDismissed(events.length),
            (e) => setError(String(e)),
          )
        }
      >
        Iniciar pelo Argus
      </button>
      <button type="button" title="Fechar" aria-label="Fechar aviso" onClick={() => setDismissed(events.length)}>
        <Icon name="close" size={13} />
      </button>
    </div>
  );
}
