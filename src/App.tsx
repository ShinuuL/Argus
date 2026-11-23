import { useEffect } from "react";
import { Shell } from "./shell/Shell";
import { ErrorBoundary } from "./shell/ErrorBoundary";
import { startApp, useApp, useLayout, useWorkbench } from "./store";
import { useTheme } from "./store/theme";
import { startPonte } from "./dev/ponte";
import { cenarioAtivo, montarCenario } from "./dev/galeria";
import { ReminderAlerts, startReminders } from "./views/reminders";

/** Raiz da interface: liga o estado global aos eventos do backend e monta o layout. */
export default function App() {
  const ready = useApp((s) => s.ready);
  useEffect(() => startApp(), []);
  useEffect(() => startReminders(), []);
  // Só faz algo quando o Argus subiu com ARGUS_PONTE (ver src/dev/ponte.ts).
  useEffect(() => startPonte(), []);
  // Idem: sem cenário escolhido pela ponte, não há o que montar.
  //
  // Depois de `ready`, não na montagem: quando a carga inicial termina, a Shell
  // abre uma conversa nova, e `openNewChat` leva a área para "chat". Navegar
  // antes disso é navegar para ser desfeito — a galeria fotografava a tela de
  // Conversas com nome de Servidores.
  //
  // Mas esperar *só* por `ready` não serve para os cenários de "carregando":
  // eles fabricam uma resposta que nunca chega, e um comando da carga inicial
  // pendurado é exatamente o que impede o `ready` — a galeria ficava parada em
  // Conversas. Por isso navega na montagem *e* a cada mudança de `ready`: a
  // primeira vez atende quem nunca fica pronto, a segunda desfaz o desvio do
  // `openNewChat` (que é efeito da Shell, portanto roda antes deste).
  useEffect(() => {
    const cen = cenarioAtivo();
    if (cen) montarCenario(cen, useLayout.getState().setArea, useTheme.getState().setTheme, useWorkbench.getState().openSession);
  }, [ready]);
  return (
    <ErrorBoundary>
      <Shell />
      <ReminderAlerts />
    </ErrorBoundary>
  );
}
