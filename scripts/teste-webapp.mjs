/*
 * Confere a web app do celular (crates/core/assets/remote/app.js) sem navegador.
 *
 * O `node --check` só diz que o arquivo compila; estas verificações montam de
 * fato a divisória de troca de agente, a linha de comando negado e o subtítulo
 * de contexto a partir de eventos reais, e olham o que saiu.
 *
 * Rode com: node scripts/teste-webapp.mjs
 */
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import vm from "node:vm";

const raiz = join(dirname(fileURLToPath(import.meta.url)), "..");
const fonte = readFileSync(join(raiz, "crates/core/assets/remote/app.js"), "utf8");

/* ----------------------------------------------- DOM mínimo (só o que o `h` usa) */

class No {
  constructor(tag) {
    this.tagName = tag;
    this.nodeType = 1;
    this.children = [];
    this.attrs = {};
    this.className = "";
    this._text = "";
    this.dataset = {};
    this.style = {};
    this.classList = { add() {}, remove() {}, toggle() {}, contains: () => false };
  }
  setAttribute(k, v) {
    this.attrs[k] = String(v);
  }
  getAttribute(k) {
    return this.attrs[k] ?? null;
  }
  addEventListener() {}
  append(...kids) {
    for (const k of kids) this.children.push(k);
  }
  set textContent(v) {
    this._text = String(v);
    this.children.length = 0;
  }
  get textContent() {
    return this._text + this.children.map((c) => (typeof c === "string" ? c : c.textContent)).join("");
  }
  set innerHTML(v) {
    this._text = String(v);
  }
  /** Classes deste nó e de toda a descendência. */
  classes() {
    const out = this.className ? this.className.trim().split(/\s+/) : [];
    for (const c of this.children) if (typeof c !== "string" && c.classes) out.push(...c.classes());
    return out;
  }
  /** Primeiro descendente (ou ele mesmo) com a classe pedida. */
  find(cls) {
    if (this.className.split(/\s+/).includes(cls)) return this;
    for (const c of this.children) {
      const achou = typeof c !== "string" && c.find && c.find(cls);
      if (achou) return achou;
    }
    return null;
  }
}

const doc = {
  // "loading" segura o `start()`: o script só registra o ouvinte e não roda a tela.
  readyState: "loading",
  createElement: (tag) => new No(tag),
  createElementNS: (_ns, tag) => new No(tag),
  createTextNode: (t) => String(t),
  addEventListener() {},
  documentElement: new No("html"),
  body: new No("body"),
  head: new No("head"),
  querySelector: () => null,
  querySelectorAll: () => [],
  getElementById: () => null,
};

const armazem = {
  getItem: () => null,
  setItem() {},
  removeItem() {},
};

const sandbox = {
  document: doc,
  localStorage: armazem,
  sessionStorage: armazem,
  location: { href: "https://exemplo.ts.net/", search: "", pathname: "/", protocol: "https:", host: "exemplo.ts.net" },
  navigator: { userAgent: "node", language: "pt-BR" },
  history: { replaceState() {}, pushState() {} },
  matchMedia: () => ({ matches: false, addEventListener() {} }),
  fetch: () => Promise.reject(new Error("sem rede no teste")),
  WebSocket: class {},
  setTimeout: () => 0,
  setInterval: () => 0,
  clearTimeout() {},
  clearInterval() {},
  requestAnimationFrame: () => 0,
  console,
};
sandbox.window = sandbox;
sandbox.globalThis = sandbox;

// As funções são declaradas com `const`/`function` no escopo do script, então
// não caem no objeto global: a expressão no fim devolve as que serão testadas.
const exportar = `;({ chatEntries, splitTurns, contextInfo, entryNode: typeof entryNode === "function" ? entryNode : null, S });`;
const ctx = vm.createContext(sandbox);
const api = vm.runInContext(fonte + exportar, ctx, { filename: "app.js" });

/* ----------------------------------------------------------------- verificações */

let falhas = 0;
function ok(nome, condicao, detalhe = "") {
  if (condicao) {
    console.log(`  ok   ${nome}`);
  } else {
    falhas++;
    console.log(`  FALHA ${nome}${detalhe ? ` — ${detalhe}` : ""}`);
  }
}

const ev = (e) => e;

/* 1. Divisória de troca de agente */
{
  const eventos = [
    ev({ User: "oi" }),
    ev({ Text: "olá" }),
    ev({ ProviderSwitch: { from: "claude", to: "opencode" } }),
    ev({ User: "e agora" }),
    ev({ Text: "pronto" }),
  ];
  const entradas = api.chatEntries(eventos, null);
  const troca = entradas.find((e) => e.type === "switch");
  ok("troca de agente vira uma entrada 'switch'", !!troca, JSON.stringify(entradas.map((e) => e.type)));
  ok("a divisória guarda de quem para quem", troca && troca.from === "claude" && troca.to === "opencode", JSON.stringify(troca));

  // Trocas seguidas sem nada no meio viram uma divisória só.
  const seguidas = api.chatEntries(
    [ev({ User: "oi" }), ev({ ProviderSwitch: { from: "claude", to: "opencode" } }), ev({ ProviderSwitch: { from: "opencode", to: "gemini" } })],
    null,
  );
  const trocas = seguidas.filter((e) => e.type === "switch");
  ok("duas trocas seguidas viram uma divisória só", trocas.length === 1, JSON.stringify(trocas));
  ok("a divisória única vai do primeiro ao último agente", trocas[0] && trocas[0].from === "claude" && trocas[0].to === "gemini", JSON.stringify(trocas[0]));

  // Ida e volta se anula (voltou para o agente de onde saiu).
  const volta = api.chatEntries(
    [ev({ User: "oi" }), ev({ ProviderSwitch: { from: "claude", to: "opencode" } }), ev({ ProviderSwitch: { from: "opencode", to: "claude" } })],
    null,
  );
  ok("ida e volta para o mesmo agente não deixa divisória", volta.filter((e) => e.type === "switch").length === 0, JSON.stringify(volta));

  // A divisória fica entre os turnos, nunca dentro de "N passos".
  const turnos = api.splitTurns(entradas);
  const comTroca = turnos.filter((t) => t.items.some((i) => i.type === "switch"));
  ok("a divisória fica sozinha no seu turno", comTroca.length === 1 && comTroca[0].items.length === 1 && comTroca[0].user == null, JSON.stringify(turnos.map((t) => t.items.map((i) => i.type))));
}

/* 2. Linha de comando negado */
{
  const entradas = api.chatEntries([ev({ User: "roda o git" }), ev({ Denied: { tool: "PowerShell", command: "git status" } })], null);
  const neg = entradas.find((e) => e.type === "denied");
  ok("ferramenta negada vira uma entrada 'denied'", !!neg, JSON.stringify(entradas.map((e) => e.type)));
  ok("guarda a ferramenta e o comando", neg && neg.tool === "PowerShell" && neg.command === "git status", JSON.stringify(neg));

  // `Denied` sem comando (ferramenta que não é de shell) não perde a ferramenta.
  const semCmd = api.chatEntries([ev({ Denied: { tool: "WebFetch" } })], null).find((e) => e.type === "denied");
  ok("negada sem comando mantém o nome da ferramenta", semCmd && semCmd.tool === "WebFetch" && semCmd.command === "", JSON.stringify(semCmd));
}

/* 3. Subtítulo com a ocupação da janela de contexto */
{
  const vazio = api.contextInfo(null);
  ok("sem dado de contexto o subtítulo fica vazio", Array.isArray(vazio) && vazio.length === 0, JSON.stringify(vazio));

  // Sem tamanho de janela conhecido: só os tokens, em pt-BR.
  const semJanela = api.contextInfo({ input_tokens: 12000, cache_read: 500, cache_write: 0 });
  ok("sem janela conhecida mostra os tokens", semJanela.length === 1 && semJanela[0].includes("contexto"), JSON.stringify(semJanela));
  ok(
    "números do subtítulo em pt-BR (ponto de milhar)",
    semJanela[0].includes("12.500"),
    `esperava 12.500 em ${JSON.stringify(semJanela[0])}`,
  );

  // Com janela: porcentagem, e o selo muda de nível conforme enche.
  const nivel = (c) => {
    const saida = api.contextInfo(c);
    const span = saida.find((x) => x && x.className !== undefined);
    return span ? { cls: span.className.trim(), texto: span.textContent, title: span.getAttribute("title") } : null;
  };
  const calmo = nivel({ input_tokens: 50000, cache_read: 0, cache_write: 0, window: 200000 });
  ok("25% não ganha aviso", calmo && calmo.cls === "ctx" && calmo.texto === "contexto 25%", JSON.stringify(calmo));
  const atencao = nivel({ input_tokens: 170000, cache_read: 0, cache_write: 0, window: 200000 });
  ok("85% avisa 'acima de 80%'", atencao && atencao.cls === "ctx warn" && atencao.texto.includes("acima de 80%"), JSON.stringify(atencao));
  const cheio = nivel({ input_tokens: 196000, cache_read: 0, cache_write: 0, window: 200000, model: "opus" });
  ok("98% avisa 'quase cheio'", cheio && cheio.cls === "ctx err" && cheio.texto.includes("quase cheio"), JSON.stringify(cheio));
  ok("o tooltip traz os números em pt-BR e o modelo", cheio && cheio.title === "196.000 de 200.000 tokens · opus", JSON.stringify(cheio && cheio.title));

  // O total soma entrada + cache lido + cache gravado (100 + 50 + 10 = 160 mil de 200 mil).
  const somado = nivel({ input_tokens: 100000, cache_read: 50000, cache_write: 10000, window: 200000 });
  ok("o total soma entrada e cache", somado && somado.title.startsWith("160.000 de 200.000"), JSON.stringify(somado));
  ok("80% já conta como aviso", somado && somado.cls === "ctx warn", JSON.stringify(somado));
}

console.log(falhas === 0 ? "\nweb app do celular: tudo certo" : `\nweb app do celular: ${falhas} falha(s)`);
process.exit(falhas === 0 ? 0 : 1);
