#!/usr/bin/env node
/**
 * Operar a janela do Argus pela linha de comando (ponte de validação).
 *
 * Existe porque nem `SendKeys` nem clique sintético do Win32 chegam ao
 * WebView2: de fora dá para fotografar a janela, não para mexer nela. A ponte
 * entra por dentro — ver src-tauri/src/ponte.rs e src/dev/ponte.ts.
 *
 * Suba o Argus com a ponte ligada:
 *
 *   $env:ARGUS_PONTE = "1"; npm run tauri dev
 *
 * E então:
 *
 *   node scripts/ui.mjs ver
 *   node scripts/ui.mjs clicar "Configurações"
 *   node scripts/ui.mjs digitar --seletor "input[aria-label='Nova tarefa']" "comprar pão"
 *   node scripts/ui.mjs tecla k --ctrl
 *   node scripts/ui.mjs esperar "Tudo certo" --ms 15000
 *   node scripts/ui.mjs erros
 *   node scripts/ui.mjs js "document.querySelectorAll('.ap-mural-item').length"
 *
 * Saída: JSON na saída padrão. Código 1 quando a ação falhou — dá para
 * encadear num roteiro de validação e parar no primeiro erro.
 */
const PORTA = process.env.ARGUS_PONTE && /^\d+$/.test(process.env.ARGUS_PONTE) ? Number(process.env.ARGUS_PONTE) : 8787;
const BASE = `http://127.0.0.1:${PORTA}`;

const AJUDA = `uso: node scripts/ui.mjs <ação> [args]

  ver                          o que está na tela (texto e o que é clicável)
  clicar <texto>               clica pelo rótulo (ou --seletor <css>)
  digitar <texto>              escreve no campo com foco (ou --seletor <css>)
  tecla <tecla> [--ctrl] [--shift] [--alt]
  esperar <texto> [--ms N]     espera o texto aparecer
  erros                        erros de console desde que a janela abriu
  js <expressão>               avalia na página e devolve o valor

Liga a ponte subindo o Argus com ARGUS_PONTE=1 (ou com a porta desejada).`;

const argv = process.argv.slice(2);
if (argv.length === 0 || argv[0] === "--ajuda" || argv[0] === "-h") {
  console.log(AJUDA);
  process.exit(argv.length === 0 ? 1 : 0);
}

// Separa as opções (--nome [valor]) dos argumentos soltos.
const opcoes = {};
const soltos = [];
for (let i = 0; i < argv.length; i++) {
  const a = argv[i];
  if (!a.startsWith("--")) {
    soltos.push(a);
    continue;
  }
  const nome = a.slice(2);
  const proximo = argv[i + 1];
  if (proximo === undefined || proximo.startsWith("--")) {
    opcoes[nome] = true;
  } else {
    opcoes[nome] = /^\d+$/.test(proximo) ? Number(proximo) : proximo;
    i++;
  }
}

const [acao, ...resto] = soltos;
const texto = resto.join(" ");
const corpo = { acao, ...opcoes };
if (texto) {
  if (acao === "tecla") corpo.tecla = texto;
  else if (acao === "js") corpo.codigo = texto;
  else corpo.texto = texto;
}

let resposta;
try {
  const r = await fetch(`${BASE}/cmd`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(corpo),
  });
  resposta = await r.json();
} catch (e) {
  // Causa quase certa: o Argus não está no ar, ou subiu sem ARGUS_PONTE.
  console.error(`não consegui falar com a ponte em ${BASE}: ${e.message}`);
  console.error("o Argus está aberto? subiu com ARGUS_PONTE definida?");
  process.exit(2);
}

if (resposta?.erro) {
  console.error(resposta.erro);
  process.exit(1);
}
console.log(JSON.stringify(resposta?.resultado ?? resposta, null, 2));
