/**
 * Barra de ícones fixa (48 px, .rail do mockup): troca a área visível.
 * Configurações fica embaixo. "Agentes" não tem ícone próprio: abre pelo
 * inspetor da conversa ou pela paleta.
 */
import type { LucideIcon } from "lucide-react";
import {
  CalendarClock,
  ChartColumn,
  GitPullRequestArrow,
  House,
  MessageSquare,
  Server,
  Settings,
  Smartphone,
  SquareTerminal,
} from "lucide-react";
import { useLayout, type Area } from "@/store";

const TOP: { id: Area; label: string; icon: LucideIcon }[] = [
  { id: "chat", label: "Conversas", icon: MessageSquare },
  { id: "home", label: "Início", icon: House },
  // Pull requests abre a área; o git-branch ficava pesado embaixo e à esquerda,
  // torto ao lado dos ícones simétricos da barra.
  { id: "git", label: "Git", icon: GitPullRequestArrow },
  { id: "servers", label: "Servidores", icon: Server },
  { id: "devices", label: "Dispositivos", icon: Smartphone },
  { id: "metrics", label: "Métricas", icon: ChartColumn },
  { id: "terminal", label: "Terminal", icon: SquareTerminal },
  { id: "tasks", label: "Tarefas agendadas", icon: CalendarClock },
];

function RailItem({ id, label, icon: I }: { id: Area; label: string; icon: LucideIcon }) {
  const on = useLayout((s) => s.area === id || (id === "chat" && s.area === "agents"));
  return (
    <button
      className={on ? "ri on" : "ri"}
      aria-current={on ? "page" : undefined}
      aria-label={label}
      title={label}
      onClick={() => useLayout.getState().setArea(id)}
    >
      {/* 17 px abre o miolo dos anéis e das fendas (servidor, PR) sem engordar o traço. */}
      <I size={17} strokeWidth={1.7} aria-hidden="true" />
    </button>
  );
}

export function NavRail() {
  return (
    <nav className="rail" aria-label="Áreas">
      {TOP.map((a) => (
        <RailItem key={a.id} {...a} />
      ))}
      <span className="rail-gap" />
      <RailItem id="settings" label="Configurações" icon={Settings} />
    </nav>
  );
}
