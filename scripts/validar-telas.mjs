#!/usr/bin/env node
/**
 * Passa por todas as áreas do Argus pela ponte de validação e diz o que quebrou.
 *
 * É o roteiro que antes só uma pessoa conseguia fazer, clicando. Para cada
 * área: clica no ícone da barra esquerda, espera a tela trocar, conta o que
 * ficou visível e recolhe os erros de console que apareceram no caminho.
 *
 *   $env:ARGUS_PONTE = "1"; npm run app:dev     # numa janela
 *   node scripts/validar-telas.mjs              # noutra
 *
 * Sai 1 se alguma área não abriu ou se o console reclamou.
 */
const PORTA = process.env.ARGUS_PONTE && /^\d+$/.test(process.env.ARGUS_PONTE) ? Number(process.env.ARGUS_PONTE) : 8787;

/**
 * As áreas da barra lateral: `[id, rótulo]`, iguais a `AREAS` em
 * src/store/layout.ts. A área "agents" fica de fora de propósito — ela não tem
 * ícone na barra, abre pelo inspetor ou pela paleta.
 */
const AREAS = [
  ["chat", "Conversas"],
  ["home", "Início"],
  ["git", "Git"],
  ["servers", "Servidores"],
  ["devices", "Dispositivos"],
  ["metrics", "Métricas"],
  ["terminal", "Terminal"],
  ["tasks", "Tarefas agendadas"],
  ["settings", "Configurações"],
];

/** Texto curto demais para ser uma tela: quase sempre é uma área que não montou. */
const TEXTO_MIN = 40;

async function ponte(corpo) {
  const r = await fetch(`http://127.0.0.1:${PORTA}/cmd`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(corpo),
  });
  return r.json();
}

let falhas = 0;
try {
  await ponte({ acao: "tecla", tecla: "Escape" });
} catch (e) {
  console.error(`a ponte não respondeu na porta ${PORTA}: ${e.message}`);
  console.error('o Argus está aberto com ARGUS_PONTE definida? ($env:ARGUS_PONTE = "1"; npm run app:dev)');
  process.exit(2);
}

for (const [id, rotulo] of AREAS) {
  const clique = await ponte({ acao: "clicar", texto: rotulo });
  if (clique.erro) {
    console.log(`✘ ${rotulo}: ${clique.erro}`);
    falhas++;
    continue;
  }
  const tela = await ponte({ acao: "ver" });
  const texto = tela.resultado?.texto ?? "";
  const botoes = tela.resultado?.clicaveis?.length ?? 0;
  // Conferir a área, não só que "apareceu texto". Antes deste teste o roteiro
  // dava `✔ Agentes` para um clique que não tinha navegado para lugar nenhum:
  // o rótulo bateu com um botão de outra tela e o texto, claro, existia.
  if (tela.resultado?.area !== id) {
    console.log(`✘ ${rotulo}: clicou mas a área continua "${tela.resultado?.area ?? "?"}"`);
    falhas++;
  } else if (texto.length < TEXTO_MIN) {
    console.log(`✘ ${rotulo}: abriu quase vazia (${texto.length} caracteres)`);
    falhas++;
  } else {
    console.log(`✔ ${rotulo}: ${botoes} elementos clicáveis, ${texto.length} caracteres de texto`);
  }
}

const erros = (await ponte({ acao: "erros" })).resultado ?? [];
if (erros.length > 0) {
  console.log(`\n${erros.length} erro(s) de console no caminho:`);
  for (const e of erros) console.log(`  - ${e.texto}`);
  falhas += erros.length;
}

console.log(falhas === 0 ? "\ntodas as telas abriram, console limpo" : `\n${falhas} problema(s)`);
process.exit(falhas === 0 ? 0 : 1);
