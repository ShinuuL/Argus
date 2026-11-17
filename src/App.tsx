import { useEffect } from "react";
import { Shell } from "./shell/Shell";
import { ErrorBoundary } from "./shell/ErrorBoundary";
import { startApp } from "./store";

/** Raiz da interface: liga o estado global aos eventos do backend e monta o layout. */
export default function App() {
  useEffect(() => startApp(), []);
  return (
    <ErrorBoundary>
      <Shell />
    </ErrorBoundary>
  );
}
