import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { instalarGaleria } from "./dev/galeria";
import "@fontsource-variable/geist/wght.css";
import "./styles/index.css";
import "./ui/ui.css";

// Antes do primeiro render, de propósito: as áreas disparam seus comandos no
// efeito de montagem, e a galeria precisa já estar no caminho. Sem um cenário
// escolhido pela ponte, esta linha não faz nada (ver src/dev/galeria.ts).
instalarGaleria();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
