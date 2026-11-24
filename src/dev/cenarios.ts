/**
 * Cenários da galeria de estados (ver `galeria.ts`).
 *
 * Cada cenário descreve **o que o backend responde**, não o que a tela desenha.
 * Isso é de propósito: a tela renderizada é a de verdade, com os mesmos
 * componentes, o mesmo CSS e os mesmos efeitos — só os dados são fabricados.
 * Um "estado vazio" aqui é o estado vazio que o usuário vê, não uma imitação.
 *
 * Três respostas possíveis para um comando:
 *
 *   { valor }   responde isso
 *   "demora"    nunca responde  → a tela fica em "carregando" para sempre
 *   { erro }    rejeita         → a tela mostra o caminho de erro
 *
 * Comando que o cenário não menciona passa direto para o backend real. Por isso
 * o app continua vivo (janela, tema, ponte) mesmo com meia dúzia de comandos
 * fabricados.
 */
import type { MetricsReport, Project, RootInfo, Schedule, ServerInfo, Task } from "@/api";
import type { Area } from "@/store";
import type { ThemeId } from "@/store/theme";

export type Resposta = { valor: unknown } | "demora" | { erro: string };

export interface Cenario {
  /** Usado em `node scripts/galeria.mjs <id>` e no nome do arquivo da foto. */
  id: string;
  rotulo: string;
  /** Área que a galeria abre sozinha ao carregar. */
  area: Area;
  /** Tema forçado; ausente = o que estiver escolhido. */
  tema?: ThemeId;
  /** Largura da janela em px; ausente = 1400. */
  largura?: number;
  /**
   * Conversa a abrir como aba, para os cenários de Conversas.
   *
   * A área "chat" sozinha mostra sempre a conversa nova e vazia — é o estado em
   * que o app abre, e era o único que a galeria sabia fotografar. Com o id de
   * uma tarefa das fixtures, a aba abre com histórico e a tela de Conversas
   * cheia (cabeçalho da sessão, linha do tempo, compositor) entra na foto.
   */
  sessao?: number;
  respostas: Record<string, Resposta>;
  /**
   * Eventos do backend que este cenário recusa ouvir.
   *
   * Fabricar a resposta do comando não basta para as telas que também são
   * alimentadas por evento: o backend empurra a realidade logo depois do boot
   * e apaga a fixture. Calar `servers-changed` é o que mantém a lista cheia
   * cheia.
   */
  calar?: string[];
  /**
   * O que a validação exige ver na tela. Texto puro procura no `innerText`;
   * `@` na frente quer dizer seletor CSS. Sem isto, "a foto saiu" provaria só
   * que a área trocou — uma fixture que carregou vazia passaria.
   */
  esperar?: string[];
}

// ------------------------------------------------------------------ fixtures

const RAIZ = "D:/Dev/Projetos";

const projeto = (name: string): Project => ({ name, path: `${RAIZ}/${name}`, kinds: ["Node"] });

const RAIZ_CHEIA: RootInfo = {
  root: RAIZ,
  projects: ["loja-api", "loja-web", "painel", "integracao-fiscal", "app-mobile", "scripts"].map(projeto),
  watch_error: null,
};

/**
 * Agora, de verdade.
 *
 * Era um instante fixo, para que duas fotos do mesmo cenário fossem iguais ao
 * pixel. Só que o app mostra *tempo relativo*: um mês depois de escrito, o
 * instante fixo virou "No ar há 8775h13" na foto de Servidores — um número
 * absurdo, que estraga a foto muito mais do que a instabilidade que ele
 * evitava. As durações daqui são deslocamentos a partir deste ponto.
 */
const AGORA = Math.floor(Date.now() / 1000);

const tarefa = (id: number, prompt: string, state: Task["state"], project: string): Task => ({
  id,
  project: `${RAIZ}/${project}`,
  provider: "claude",
  prompt,
  state,
  workdir: null,
  session_id: `sess-${id}`,
  turns: 3,
  model: null,
  mode: "Direct",
  handoff: false,
  access: "normal",
  archived: false,
  fork_of: null,
});

const TAREFAS: Task[] = [
  tarefa(31, "Corrigir o cálculo de ICMS ST na emissão", "Running", "integracao-fiscal"),
  tarefa(30, "Migrar o painel para a nova paleta", "Running", "painel"),
  tarefa(29, "Teste de ponta a ponta do checkout", "Queued", "loja-web"),
  tarefa(28, "Subir o servidor de desenvolvimento", "Completed", "loja-api"),
  tarefa(27, "Revisar o contrato da API de estoque", { Failed: "a CLI saiu com código 1" }, "loja-api"),
];

const servidor = (pid: number, name: string, port: number, cwd: string): ServerInfo => ({
  pid,
  name,
  cmdline: `${name} run dev`,
  port,
  url: `http://localhost:${port}`,
  owner: { kind: "Session", id: 30 },
  started_at: AGORA - 3600,
  cwd: `${RAIZ}/${cwd}`,
  source: "session",
  exposure: null,
});

const SERVIDORES: ServerInfo[] = [
  servidor(12001, "node", 5173, "loja-web"),
  servidor(12002, "node", 3000, "loja-api"),
  servidor(12003, "python", 8000, "integracao-fiscal"),
  { ...servidor(12004, "node", 4321, "painel"), source: "folder" },
  {
    ...servidor(12005, "node", 8080, "app-mobile"),
    exposure: { kind: "cloudflared", label: "Cloudflare", pid: 12099, url: "https://exemplo.trycloudflare.com" },
  },
];

const agenda = (id: number, name: string, enabled: boolean, last: Schedule["last_status"]): Schedule => ({
  id,
  name,
  kind: "Agent",
  project: `${RAIZ}/loja-api`,
  provider: "claude",
  model: null,
  mode: null,
  prompt: "Rodar os testes e abrir uma conversa se algum quebrar",
  recurrence: { kind: "Daily", time: "07:30" },
  enabled,
  created_at: AGORA - 86_400 * 30,
  last_run: AGORA - 86_400,
  next_run: AGORA + 3600,
  last_task: 28,
  last_status: last,
  last_late: false,
});

const AGENDAS: Schedule[] = [
  agenda(1, "Testes da manhã", true, "Completed"),
  agenda(2, "Varredura de dependências", true, { Failed: "npm audit saiu com código 1" }),
  agenda(3, "Resumo da semana", false, null),
];

const linha = (key: string, mult: number) => ({
  key,
  input_tokens: 120_000 * mult,
  output_tokens: 18_000 * mult,
  cache_read: 900_000 * mult,
  cache_write: 60_000 * mult,
  turns: 40 * mult,
  api_cost_usd: 3.2 * mult,
});

/** Quatorze dias com variação real: é o que mostra se a escala do gráfico presta. */
const DIAS = [31, 47, 12, 55, 38, 61, 44, 29, 50, 66, 41, 35, 58, 49].map((v, i) => ({
  day: `2026-11-${String(i + 8).padStart(2, "0")}`,
  line: linha("", v / 10),
}));

const RELATORIO: MetricsReport = {
  total: linha("total", 56),
  projects: [linha(`${RAIZ}/loja-api`, 22), linha(`${RAIZ}/painel`, 15), linha(`${RAIZ}/integracao-fiscal`, 11), linha(`${RAIZ}/loja-web`, 8)],
  providers: [linha("claude", 44), linha("codex", 12)],
  models: [linha("sonnet", 38), linha("haiku", 12), linha("opus", 6)],
  days: DIAS,
  has_db: true,
  insights: {
    cache_hit_rate: 0.82,
    split: { input: 12.4, cache_write: 9.1, cache_read: 4.8, output: 31.2 },
    tasks: [31, 30, 29, 28, 27].map((id) => ({
      task_id: id,
      project: "loja-api",
      title: TAREFAS.find((t) => t.id === id)?.prompt ?? "",
      cost: 12.5 - id * 0.3,
      turns: 18,
      cost_per_turn: 0.7,
      max_context: 148_000,
      cache_hit_rate: 0.8,
    })),
    rewrite_turns: 4,
    rewrite_write_cost: 1.9,
    rewrites: [],
  },
};

/**
 * Histórico da conversa 31, com um pouco de cada linha que a tela sabe desenhar:
 * pedido, raciocínio, ferramenta que deu certo, ferramenta que falhou, aviso e
 * resposta final. Uma conversa só de texto não mostraria nada do que a linha do
 * tempo faz.
 */
const HISTORICO: unknown[] = [
  { User: "O ICMS ST está vindo zerado na emissão quando o produto tem redução de base. Investiga e corrige." },
  { Thinking: "Primeiro achar onde a base reduzida entra no cálculo; o zero provavelmente vem de uma divisão por uma alíquota ainda não carregada." },
  { ToolUse: { name: "Grep", input: { pattern: "icms_st", path: "src/fiscal" } } },
  { ToolResult: { ok: true, output: "src/fiscal/calculo.rs:142\nsrc/fiscal/calculo.rs:198\nsrc/fiscal/testes.rs:77" } },
  { Text: "A redução de base é aplicada **depois** do cálculo do ST, em `calculo.rs:198`. Com redução, a base que chega ao ST já foi zerada." },
  { ToolUse: { name: "Edit", input: { file_path: "src/fiscal/calculo.rs" } } },
  { ToolResult: { ok: true, output: "1 alteração aplicada" } },
  { ToolUse: { name: "Bash", input: { command: "cargo test -p fiscal" } } },
  { ToolResult: { ok: false, output: "test st_com_reducao ... FAILED\nesperado 42.18, veio 42.17" } },
  { Notice: { level: "warn", text: "O teste compara centavos com igualdade exata." } },
  { ToolUse: { name: "Bash", input: { command: "cargo test -p fiscal" } } },
  { ToolResult: { ok: true, output: "test result: ok. 34 passed; 0 failed" } },
  { Text: "Corrigido: a redução passou a entrar antes do ST e o arredondamento virou meio-para-cima em duas casas, como manda a nota técnica. Os 34 testes do módulo fiscal passam." },
  { Usage: { input_tokens: 18_400, output_tokens: 3_100, cache_read: 212_000, cache_write: 14_900 } },
];

/**
 * Aparelhos conectados.
 *
 * A moldura do Screen-Mirror fica vazia na foto: o painel é outro aplicativo,
 * servido por um sidecar que não está no ar durante a galeria, e não há como
 * fabricar o conteúdo de um iframe pelo `invoke`. O cenário existe para o que é
 * do Argus — a lista de aparelhos, o cabeçalho com o modelo, a legenda da
 * moldura e a ficha à direita.
 */
const MIRROR_NO_AR = {
  dir: "D:/Dev/Projetos/Screen-Mirror",
  node: "22.18.0",
  nodeTooOld: false,
  needsInstall: false,
  url: "http://127.0.0.1:7331",
  pid: 15_440,
  error: null,
  agentsMcp: true,
};

const APARELHOS = {
  devices: [
    { serial: "R58N70PQRST", state: "device", model: "Galaxy S23", connection: "usb" },
    { serial: "192.168.0.42:5555", state: "device", model: "Redmi Note 12", connection: "wifi" },
    { serial: "ZY22GHJK9L", state: "unauthorized", model: null, connection: "usb" },
  ],
  mirroring: ["R58N70PQRST"],
};

// ------------------------------------------------------------------ cenários

/**
 * Tudo vazio: nenhuma pasta raiz, nenhuma sessão, nada rodando.
 *
 * `workspace_current` é o que a carga inicial chama (`bootstrap.ts`);
 * `workspace_refresh` é o do botão "Atualizar" e do foco da janela. Os dois
 * precisam responder igual, senão a tela muda sozinha quando alguém volta para
 * a janela no meio da sessão de fotos.
 */
const VAZIO: Record<string, Resposta> = {
  workspace_current: { valor: null },
  workspace_refresh: { valor: null },
  tasks_list: { valor: [] },
  servers_list: { valor: [] },
  schedules_list: { valor: [] },
};

/** Backend que nunca responde: é o estado "carregando" de cada tela. */
const DEMORA = (...cmds: string[]): Record<string, Resposta> =>
  Object.fromEntries(cmds.map((c) => [c, "demora" as Resposta]));

const ERRO = (msg: string, ...cmds: string[]): Record<string, Resposta> =>
  Object.fromEntries(cmds.map((c) => [c, { erro: msg } as Resposta]));

const CHEIO: Record<string, Resposta> = {
  workspace_current: { valor: RAIZ_CHEIA },
  workspace_refresh: { valor: RAIZ_CHEIA },
  workspace_open_root: { valor: RAIZ_CHEIA },
  monitor_statuses: { valor: [] },
  tasks_list: { valor: TAREFAS },
  servers_list: { valor: SERVIDORES },
  schedules_list: { valor: AGENDAS },
  metrics_report: { valor: RELATORIO },
};

/**
 * Eventos calados em todo cenário (ver `Cenario.calar`).
 *
 * São os que empurram a realidade por cima do que foi fabricado. Ficam de fora
 * os do terminal (`pty-*`) e os de mídia: nenhum cenário mexe com eles, e calar
 * o que não atrapalha só esconderia defeito de verdade na hora da foto.
 */
const CALAR = ["servers-changed", "schedules-changed", "workspace-changed", "project-changed", "runs-changed", "run-finished", "orch"];

const lista = (cs: Omit<Cenario, "calar">[]): Cenario[] => cs.map((c) => ({ ...c, calar: CALAR }));

/** O que a aba de conversa lê ao abrir a tarefa 31 das fixtures. */
const CONVERSA: Record<string, Resposta> = {
  ...CHEIO,
  task_get: { valor: TAREFAS[0] },
  task_history: { valor: HISTORICO },
  task_todos: { valor: [] },
  task_checkpoints: { valor: [] },
  task_subagents: { valor: [] },
  task_subagent_times: { valor: [] },
  task_effort: { valor: null },
  task_idle_secs: { valor: null },
};

/**
 * O agente escreveu uma pergunta com opções e errou o JSON (foi o que aconteceu
 * de verdade em 2026-11-21: faltou a chave final). Antes, o bloco inteiro saía
 * pelo caminho do markdown e o usuário levava mil caracteres de JSON cru sem
 * pergunta nenhuma; agora vira um aviso de uma linha (ver `ask.ts`).
 */
const ASK_QUEBRADO: Record<string, Resposta> = {
  ...CONVERSA,
  task_get: { valor: { ...TAREFAS[0], state: "Completed" as Task["state"] } },
  task_history: {
    valor: [
      { User: "Terminou o passo 3? Me diz o que vem depois." },
      {
        Text:
          "As quatro telas estão feitas. Sobre o que vem depois:\n\n" +
          '```argus-ask\n{"questions":[{"header":"Próximo passo","question":"Por onde seguir?","options":[{"label":"Comparar as telas","description":"Diff contra os prints antigos"}]}]\n```',
      },
      { Result: { ok: true, summary: "", cost: null } },
    ],
  },
};

const DISPOSITIVOS: Record<string, Resposta> = {
  ...VAZIO,
  devices_status: { valor: MIRROR_NO_AR },
  devices_list: { valor: APARELHOS },
  devices_apks: { valor: [] },
};

export const CENARIOS: Cenario[] = lista([
  // ---- vazio (o que os prints de 2026-11-21 já mostraram, agora reproduzível)
  { id: "inicio-vazio", rotulo: "Início · vazio", area: "home", respostas: VAZIO, esperar: ["@[data-ponte='vazio']", "Abrir pasta raiz"] },
  { id: "conversas-vazio", rotulo: "Conversas · sem pasta raiz", area: "chat", respostas: VAZIO, esperar: ["Comece abrindo seus projetos", "Abrir pasta raiz"] },
  { id: "dispositivos-vazio", rotulo: "Dispositivos · sidecar parado", area: "devices", respostas: { ...VAZIO, devices_status: { valor: { ...MIRROR_NO_AR, url: null, pid: null } } }, esperar: ["Nenhum celular conectado por ADB", "Screen-Mirror"] },
  { id: "git-vazio", rotulo: "Git · vazio", area: "git", respostas: VAZIO, esperar: ["@[data-ponte='vazio']", "Nenhum projeto com git na pasta raiz"] },
  { id: "servidores-vazio", rotulo: "Servidores · vazio", area: "servers", respostas: VAZIO, esperar: ["@[data-ponte='vazio']", "Nenhum servidor em execução"] },
  { id: "agendadas-vazio", rotulo: "Agendadas · vazio", area: "tasks", respostas: VAZIO, esperar: ["@[data-ponte='vazio']", "Nenhuma tarefa agendada"] },
  { id: "metricas-vazio", rotulo: "Métricas · sem histórico", area: "metrics", respostas: { ...VAZIO, metrics_report: { valor: { ...RELATORIO, has_db: false, days: [], projects: [], providers: [], models: [], total: linha("total", 0) } } }, esperar: ["Histórico desligado", "Nenhum uso registrado no período"] },

  // ---- carregando (a área já prova que a tela montou; o texto muda de tela para tela)
  { id: "inicio-carregando", rotulo: "Início · carregando", area: "home", respostas: DEMORA("workspace_current", "workspace_refresh", "tasks_list", "schedules_list", "project_checklist") },
  { id: "servidores-carregando", rotulo: "Servidores · carregando", area: "servers", respostas: DEMORA("servers_list") },
  { id: "metricas-carregando", rotulo: "Métricas · carregando", area: "metrics", respostas: DEMORA("metrics_report") },
  { id: "agendadas-carregando", rotulo: "Agendadas · carregando", area: "tasks", respostas: DEMORA("schedules_list") },

  // ---- erro
  { id: "servidores-erro", rotulo: "Servidores · erro", area: "servers", respostas: ERRO("não consegui listar as portas: acesso negado", "servers_list"), esperar: ["acesso negado"] },
  { id: "metricas-erro", rotulo: "Métricas · erro", area: "metrics", respostas: ERRO("banco de histórico corrompido (database disk image is malformed)", "metrics_report"), esperar: ["banco de histórico corrompido"] },
  { id: "agendadas-erro", rotulo: "Agendadas · erro", area: "tasks", respostas: ERRO("não consegui ler as tarefas agendadas", "schedules_list"), esperar: ["não consegui ler as tarefas agendadas"] },
  { id: "dispositivos-erro", rotulo: "Dispositivos · ADB recusando", area: "devices", respostas: ERRO("adb: device unauthorized. Confirme a depuração USB no aparelho.", "devices_list", "devices_status"), esperar: ["device unauthorized"] },

  // ---- cheio
  { id: "inicio-cheio", rotulo: "Início · cheio", area: "home", respostas: CHEIO, esperar: ["3 agentes rodando", "loja-api"] },
  { id: "servidores-cheio", rotulo: "Servidores · cheio", area: "servers", respostas: CHEIO, esperar: ["5 em execução", "loja-api"] },
  { id: "agendadas-cheio", rotulo: "Agendadas · cheio", area: "tasks", respostas: CHEIO, esperar: ["Testes da manhã", "Varredura de dependências"] },
  { id: "metricas-cheio", rotulo: "Métricas · 14 dias", area: "metrics", respostas: CHEIO, esperar: ["Tokens por dia", "66%"] },
  { id: "conversas-cheio", rotulo: "Conversas · com histórico", area: "chat", sessao: 31, respostas: CONVERSA, esperar: ["Corrigir o cálculo de ICMS ST", "Os 34 testes do módulo fiscal passam"] },
  { id: "conversas-ask-quebrado", rotulo: "Conversas · pergunta ilegível", area: "chat", sessao: 31, respostas: ASK_QUEBRADO, esperar: ["não deu para ler"] },
  { id: "dispositivos-cheio", rotulo: "Dispositivos · três aparelhos", area: "devices", respostas: DISPOSITIVOS, esperar: ["Galaxy S23", "Redmi Note 12"] },

  // ---- janela estreita e os outros três temas, sobre a tela mais densa
  { id: "servidores-estreito", rotulo: "Servidores · 900 px", area: "servers", largura: 900, respostas: CHEIO, esperar: ["5 em execução"] },
  { id: "inicio-estreito", rotulo: "Início · 900 px", area: "home", largura: 900, respostas: CHEIO, esperar: ["3 agentes rodando"] },
  { id: "metricas-ciano", rotulo: "Métricas · Ardósia", area: "metrics", tema: "ciano", respostas: CHEIO, esperar: ["Tokens por dia"] },
  { id: "metricas-ambar", rotulo: "Métricas · Grafite", area: "metrics", tema: "ambar", respostas: CHEIO, esperar: ["Tokens por dia"] },
  { id: "metricas-violeta", rotulo: "Métricas · Noite", area: "metrics", tema: "violeta", respostas: CHEIO, esperar: ["Tokens por dia"] },
]);

export const acharCenario = (id: string): Cenario | undefined => CENARIOS.find((c) => c.id === id);
