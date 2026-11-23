/*
 * Faz o papel da câmera que lê o QR: serve, numa origem diferente da do Argus,
 * uma página com o link de pareamento. Tocar nesse link é uma navegação de
 * primeiro nível vindo de fora do site — o caso em que o cookie `SameSite=Strict`
 * não ia e o pareamento caía em "Não autorizado".
 *
 * `localhost` e `127.0.0.1` são hosts diferentes para o navegador, então a página
 * em `http://localhost:8766` é "outro site" em relação a `http://127.0.0.1:8765`.
 *
 * Uso: node scripts/celular-camera.mjs <url-de-pareamento> [porta]
 */
import { createServer } from "node:http";

const url = process.argv[2];
const porta = Number(process.argv[3] ?? 8766);
if (!url) {
  console.error("informe a URL de pareamento (a que sai no teste live_mobile_web_app)");
  process.exit(1);
}

const pagina = `<!doctype html>
<html lang="pt-BR"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>QR lido</title></head>
<body style="font-family:system-ui,sans-serif;padding:32px;font-size:20px">
<p>Isto faz o papel do leitor de QR: o link abaixo sai de outro site.</p>
<p><a id="abrir" href="${url}">Abrir o Argus</a></p>
</body></html>`;

createServer((_req, res) => {
  res.writeHead(200, { "content-type": "text/html; charset=utf-8", "cache-control": "no-store" });
  res.end(pagina);
}).listen(porta, "127.0.0.1", () => console.log(`câmera falsa em http://localhost:${porta}/`));
