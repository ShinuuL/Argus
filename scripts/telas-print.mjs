#!/usr/bin/env node
/**
 * Fotografa todas as telas do Argus, uma por área.
 *
 * Junta as duas metades que isoladas não serviam para nada: a ponte de
 * validação sabe **navegar** (mas só devolve texto) e o `ffmpeg -f gdigrab`
 * sabe **fotografar** (mas não sabe trocar de tela). Juntas, dão o que uma
 * crítica de design precisa — pixels de cada área, sem ninguém clicando.
 *
 *   $env:ARGUS_PONTE = "1"; npm run app:dev      # numa janela
 *   node scripts/telas-print.mjs                 # noutra
 *
 * Sai em `docs/telas/<n>-<area>.png`. O `--janela` escolhe o título capturado
 * (padrão "Argus Dev"); use `--janela Argus` para fotografar a instância
 * normal. Lembre que as duas compartilham o banco.
 */
import { execFile } from "node:child_process";
import { mkdir } from "node:fs/promises";
import { promisify } from "node:util";

const exec = promisify(execFile);
const PORTA = process.env.ARGUS_PONTE && /^\d+$/.test(process.env.ARGUS_PONTE) ? Number(process.env.ARGUS_PONTE) : 8787;
const SAIDA = "docs/telas";

/**
 * `[id da área, rótulo, nome do arquivo]`. O id é o de `AREAS` em
 * src/store/layout.ts e serve para conferir que a tela trocou mesmo. A área
 * "agents" não entra: não tem ícone na barra, abre pelo inspetor ou pela paleta.
 */
const AREAS = [
  ["chat", "Conversas", "conversas"],
  ["home", "Início", "inicio"],
  ["git", "Git", "git"],
  ["servers", "Servidores", "servidores"],
  ["devices", "Dispositivos", "dispositivos"],
  ["metrics", "Métricas", "metricas"],
  ["terminal", "Terminal", "terminal"],
  ["tasks", "Tarefas agendadas", "agendadas"],
  ["settings", "Configurações", "configuracoes"],
];

const janela = process.argv.includes("--janela") ? process.argv[process.argv.indexOf("--janela") + 1] : "Argus Dev";
/** Tamanho fixo: duas capturas só se comparam se a janela for a mesma. */
const largura = process.argv.includes("--largura") ? Number(process.argv[process.argv.indexOf("--largura") + 1]) : 1400;
const altura = process.argv.includes("--altura") ? Number(process.argv[process.argv.indexOf("--altura") + 1]) : 900;

/**
 * Um pedido à ponte. Falha de rede vira `{ erro }` em vez de exceção: a janela
 * pode fechar no meio do roteiro, e aí o que interessa é saber em que tela
 * parou — não um rastro de pilha do Node.
 */
const ponte = async (corpo) => {
  try {
    const r = await fetch(`http://127.0.0.1:${PORTA}/cmd`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(corpo),
    });
    return await r.json();
  } catch (e) {
    return { erro: `a ponte não respondeu (${e.message}) — a janela ainda está aberta?` };
  }
};

const dormir = (ms) => new Promise((r) => setTimeout(r, ms));

/**
 * Um quadro da janela: fotografa a **tela inteira** e recorta a moldura dela.
 *
 * O caminho óbvio (`-i title=Argus Dev`) não serve: com WebView2 ele devolve um
 * quadro velho — saíram nove retratos da tela de Conversas com o DOM já três
 * telas adiante. O desenho da página não passa pelo DC da janela, então o que o
 * `gdigrab` copia de lá está congelado. Da tela composta, sai o que se vê.
 */
async function fotografar(arquivo, moldura) {
  const recorte = moldura ? ["-vf", `crop=${moldura.largura}:${moldura.altura}:${moldura.x}:${moldura.y}`] : [];
  await exec("ffmpeg", ["-y", "-loglevel", "error", "-f", "gdigrab", "-framerate", "1", "-i", "desktop", ...recorte, "-frames:v", "1", arquivo]);
}

await mkdir(SAIDA, { recursive: true });
const inicio = await ponte({ acao: "tecla", tecla: "Escape" });
if (inicio.erro) {
  console.error(`${inicio.erro}`);
  console.error('suba o Argus com a ponte: $env:ARGUS_PONTE = "1"; npm run app:dev');
  process.exit(2);
}

let n = 0;
let falhas = 0;
for (const [id, rotulo, arquivoId] of AREAS) {
  n++;
  const clique = await ponte({ acao: "clicar", texto: rotulo });
  if (clique.erro) {
    console.log(`✘ ${rotulo}: ${clique.erro}`);
    falhas++;
    continue;
  }
  // Fotografar sem conferir a área rende dez retratos da mesma tela — já rendeu.
  const onde = (await ponte({ acao: "ver" })).resultado?.area;
  if (onde !== id) {
    console.log(`✘ ${rotulo}: clicou mas a área continua "${onde ?? "?"}"`);
    falhas++;
    continue;
  }
  // A janela precisa estar na frente: o `gdigrab` fotografa a região da tela,
  // então o que estiver por cima entra no retrato (já aconteceu com um Bloco
  // de Notas no meio da tela de Conversas).
  const posto = await ponte({ acao: "janela", largura, altura });
  // A troca de área tem transição: fotografar no mesmo instante pega o meio dela.
  await dormir(600);
  const arquivo = `${SAIDA}/${String(n).padStart(2, "0")}-${arquivoId}.png`;
  try {
    await fotografar(arquivo, posto.resultado?.moldura);
    console.log(`✔ ${arquivo}`);
  } catch (e) {
    console.log(`✘ ${rotulo}: ffmpeg não fotografou ("${janela}" está aberta?) — ${e.message.split("\n")[0]}`);
    falhas++;
  }
}

// Devolve a janela ao comportamento normal: "sempre à frente" é para a sessão
// de fotos, não para o resto do dia.
await ponte({ acao: "janela", fixar: false });

console.log(falhas === 0 ? `\n${n} telas em ${SAIDA}/` : `\n${falhas} de ${n} falharam`);
process.exit(falhas === 0 ? 0 : 1);
