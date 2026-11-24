/**
 * Registry das áreas em tela cheia.
 *
 * A Shell usa este mapa para montar a área visível; as abas do workbench usam
 * o mesmo mapa para abrir a tela inteira ao lado de uma conversa. Assim a lista
 * de áreas não fica duplicada e uma área nova só precisa ser registrada aqui.
 */
import type { ComponentType } from "react";
import type { Area } from "@/store/layout";
import HomeArea from "./home";
import GitArea from "./git";
import ServersArea from "./servers";
import DevicesArea from "./devices";
import MetricsArea from "./metrics";
import TasksArea from "./tasks";
import SettingsArea from "./settings";
import AgentsArea from "./agents";

/** "chat" e "terminal" têm layout próprio e não entram aqui. */
export const AREA_VIEWS: Partial<Record<Area, ComponentType>> = {
  home: HomeArea,
  git: GitArea,
  servers: ServersArea,
  devices: DevicesArea,
  metrics: MetricsArea,
  tasks: TasksArea,
  settings: SettingsArea,
  agents: AgentsArea,
};
