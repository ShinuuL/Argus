# Argus — Estado atual e próximos passos

_Atualizado em 2026-11-14, depois da migração da interface de Dioxus para Tauri 2 e do redesenho do layout (chat no centro, temas)._

O Argus é uma IDE desktop que gerencia vários projetos de uma pasta raiz e orquestra agentes de IA (Claude Code e OpenCode). A lógica fica em Rust (`argus-core`); a interface é React + TypeScript dentro de uma janela Tauri 2. Os agentes usam o login das próprias CLIs oficiais; a IDE nunca lê nem guarda tokens.

**Estado em uma frase:** tudo compila, os testes passam e as telas principais foram exercitadas na janela em 2026-11-14 (inclusive uma tarefa real do claude com subagente). A coluna "Estado" da tabela abaixo diz o que foi conferido de verdade e o que ainda falta.

A conferência na janela foi feita pelo protocolo de depuração do WebView2 (cliques e teclas enviados à página, sem usar o mouse real). Para repetir: abra o executável com `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9333` e conecte em `http://127.0.0.1:9333`. Gestos que dependem do sistema (arrastar aba, arrastar arquivo do Explorer, colar print) não passam por esse caminho e continuam sem conferência.

## Como rodar

Requisitos: Rust (stable, MSVC), Node.js 20 ou mais novo e o WebView2 (já vem no Windows 11).

```powershell
npm install                                   # uma vez, ou quando o package.json mudar
npm run tauri dev                             # abre a IDE com recarga automática da interface
npm run tauri build -- --debug --no-bundle    # gera target\debug\argus-desktop.exe sem instalador
```

Testes e verificações:

```powershell
npx tsc --noEmit            # tipos da interface
cargo test --workspace      # 68 testes do core + 15 do desktop
node --check crates/core/assets/remote/app.js   # sintaxe da web app do celular
```

A interface ainda não tem runner de testes próprio; a lógica pura das telas foi conferida por scripts avulsos durante a migração.

Variáveis úteis para testes:

| Variável | O que faz |
|---|---|
| `ARGUS_ROOT` | abre essa pasta raiz automaticamente |
| `ARGUS_DB` | usa outro banco SQLite |
| `ARGUS_ALLOWED_TOOLS` | usa outro arquivo de comandos permitidos |

Os dados ficam em `%APPDATA%\argus\`: `argus.db`, `pricing.json`, `allowed_tools.json` e a pasta `anexos\` (anexos do chat, apagados depois de 1 dia).

## Estrutura

```
package.json, vite.config.ts, index.html     front-end (vite)
src/                                         interface em React 18 + TypeScript + zustand
  api/        uma função tipada por comando Tauri, eventos e tipos que espelham o serde
  store/      useApp, useLayout, useWorkbench, useTerminals, useTheme, bootstrap (liga eventos ao estado)
  shell/      Shell (layout), TopBar, RightPanel, BottomPanel, StatusBar, ContextMenu, icons (SVG inline),
              useActiveContext (sessão e projeto da aba em foco), useGlobalShortcuts
  styles/     tokens.css (TODAS as cores, por tema), components.css (classes portadas), shell.css (layout)
  views/
    projects/   barra esquerda: projetos, sessões e o rodapé que abre as telas de apoio
    session/    a conversa (aba principal): index (ChatTab), Transcript, panels (Alterações e Subagentes
                do painel direito), SubagentTree, events (funções puras), useTaskEvents, actions
    chat/       componentes da conversa: Composer, MessageView/Markdown, fila, anexos, menções, comandos de barra
    workbench/  grupos de abas, split, Monaco (tema gerado dos tokens), Painel de projetos
    explorer/   árvore de arquivos do projeto ativo (aba Arquivos do painel direito)
    servers/    aba Servidores do painel direito, selos e "Comandos permitidos"
    agents/     aba Provedores (login e status das CLIs)
    settings/   aba Configurações (tema, auto-work por projeto, comandos permitidos)
    autowork/, deps/, metrics/, remote/, terminal/, player/, pet/
src-tauri/    crate argus-desktop (Tauri 2, identificador dev.argus.app)
  src/commands/  workspace, monitor, fs, pty, agents, autowork, servers, deps, metrics,
                 remote, media, system, extra_agents, extra_chat  (84 comandos)
  src/events.rs  repassa os eventos do orquestrador e dos servidores para a interface
crates/core   (argus-core): workspace, monitor, pty, procs, deps, metrics, db, remote, autowork,
              agents/{provider, claude, opencode, orchestrator}
              assets/remote (web app do celular: index.html, app.js, style.css, unauthorized.html,
              manifest.webmanifest, icon.svg; embutidos no executável por include_str)
crates/app    (argus-app): interface ANTIGA em Dioxus 0.7. Fora do workspace, não compila mais.
```

Monaco e xterm entram por npm e são empacotados pelo vite; nada vem de CDN.

## Layout

Inspirado na Orca (projetos e sessões à esquerda, abas no centro, arquivos à direita) e no chat do Claude Code no VS Code. Não há mais activity bar.

| Região | O que tem |
|---|---|
| Barra superior (32 px) | alternar a barra esquerda, projeto e sessão em foco, alternar terminal e painel direito |
| Barra esquerda | "Abrir pasta raiz", Painel, filtro de encerradas, recolher tudo e busca. Cada projeto (nome, branch, ±alterados) expande nas sessões: ponto de status, título (primeiro pedido), selos de auto-work e servidores, provedor. Sessão com atividade não vista fica em negrito. "+" no hover cria conversa no projeto; clique direito: Abrir, Ver diff, Cancelar o turno, Descartar. Rodapé: Provedores, Dependências, Métricas, Remoto e Configurações abrem como abas |
| Centro | grupos de abas com split (até 3). A aba de conversa é a principal: "Nova conversa" (saudação, sugestões, pílulas de projeto e provedor) vira a sessão da tarefa no primeiro envio, na mesma aba. Não existem mais as abas "Chat" e "Sessão" separadas |
| Conversa | cabeçalho com seletor de conversas (busca), "+", estado, total de tokens e custo equivalente em API. Coluna central de 820 px: pedido em caixa, resposta em Markdown, ferramentas em linhas recolhidas, "N passos" no turno terminado. Caixa de composição arredondada: anexar, "/", "@", pílulas (projeto, provedor, modo de permissão, auto-work) e Enviar, que vira Parar durante o turno. Seta para cima recupera os pedidos anteriores |
| Painel direito (retrátil) | Arquivos (projeto ativo), Alterações (diff da sessão, Aplicar e Descartar), Subagentes (árvore; abre o chat somente leitura), Servidores |
| Painel inferior | terminais (oculto por padrão; abre pelo botão Terminal, Ctrl+J ou ao criar um terminal) |
| Status bar | "N agentes rodando" (clique foca a sessão), servidores, uso de hoje; player, direção do split e pet |

Atalhos novos: Ctrl+N (nova conversa), Ctrl+B (barra esquerda), Ctrl+J (terminal); não valem com o foco num terminal.

"Projeto ativo" (`useApp.selected`) acompanha a aba em foco (`shell/useActiveContext.ts`) e pode ser trocado clicando num projeto na barra esquerda. É ele que a aba Arquivos, os terminais novos e as telas por projeto usam.

### Temas

Três temas, escolhidos em Configurações e lembrados no localStorage (`argus.tema`): **Oceano** (padrão), **Nebulosa** e **Brasa**. Toda cor vem de `src/styles/tokens.css`: cada tema define os tokens de base (`--bg`, `--surface`, `--surface-2`, `--border`, `--border-strong`, `--text`, `--text-2`, `--accent`, `--accent-hover`, `--on-accent`, `--accent-text`, `--selection`, `--focus`, `--status-bg`, `--on-status`) só com cores da própria paleta; os demais são derivados. As cores de estado (`--ok`, `--warn`, `--err`, `--run`) são iguais nos três. Nenhum componente usa hex direto. O Monaco (`views/workbench/monaco.ts`) e o xterm (`views/terminal/session.ts`) montam o tema a partir dos tokens e trocam junto.

A web app do celular (`crates/core/assets/remote/style.css`) repete os mesmos tokens e os mesmos três temas; o seletor fica na tela Ajustes dela e a escolha é guardada no aparelho (mesma chave `argus.tema`). O `theme-color`, o manifest e o ícone acompanham o tema (`/manifest.webmanifest?tema=...`).

### Acesso remoto (web app do celular)

Servidor axum em `crates/core/src/remote.rs`, desligado por padrão; liga na aba Remoto, que mostra o QR code. A web app é HTML, CSS e JavaScript puros (sem framework e sem CDN) e foi refeita em 2026-11-14 para acompanhar o desktop.

| Tela | O que tem |
|---|---|
| Pareamento | `/pair?token=...` grava o cookie (HttpOnly, SameSite=Strict) e redireciona para `/`, tirando o token do endereço. Token ausente, errado ou revogado dá a mesma página 401 "Não autorizado" |
| Sessões (início) | projetos com as suas sessões: ponto de status (erro vira ícone de alerta com o rótulo "Erro"), título, provedor, selos de auto-work e de servidores, negrito e ponto azul na sessão com novidade, busca, "Encerradas", "+" geral e por projeto |
| Conversa | pedido em caixa, resposta em Markdown (tabela com rolagem lateral, código com rótulo e Copiar), ferramentas e "N passos" recolhidos, stream pelo WebSocket, total de tokens e custo equivalente em API, caixa de composição no rodapé (acompanha o teclado), Enviar que vira Parar |
| Subagentes | lista em árvore e o chat de cada um (prompt recebido, ferramentas, resultado), somente leitura, sem caixa de resposta |
| Nova conversa | projeto (só os da pasta raiz aberta na IDE), provedor e pedido; cai na conversa criada |
| Auto-work | liga e desliga por projeto, progresso, Pausar, Retomar e Parar (com confirmação) |
| Servidores | por sessão ou terminal, com Abrir e Derrubar (com confirmação) |
| Ajustes | tema, estado da conexão, como revogar e como instalar |

Rotas (todas exigem o token, por cookie ou `Authorization: Bearer`; sem ele, 401, inclusive no WebSocket):

| Rota | O que faz |
|---|---|
| `GET /pair?token=` | pareamento (única rota sem token prévio) |
| `GET /`, `/app.js`, `/style.css`, `/icon.svg?tema=`, `/manifest.webmanifest?tema=` | a web app |
| `GET /api/tasks`, `POST /api/tasks` | lista as sessões; cria uma (só em projeto conhecido da IDE) |
| `GET /api/tasks/{id}` | a sessão e os eventos (inclui `SubagentStart`, `SubagentEnd` e `Scoped`) |
| `POST /api/tasks/{id}/reply`, `/cancel` | responde; para o turno |
| `GET /api/projects` | `[{ name, path }]`: projetos da pasta raiz aberta mais os das sessões |
| `GET /api/providers` | `[{ id, name }]` |
| `GET /api/pricing` | tabela de preços do custo equivalente em API |
| `GET /api/autowork`, `POST /api/autowork`, `POST /api/tasks/{id}/autowork/{start,pause,resume,stop}` | auto-work |
| `GET /api/servers`, `POST /api/servers/{pid}/kill` | servidores rastreados; derruba um (só PID rastreado) |
| `GET /api/stream` | WebSocket: `state`, `agent`, `stderr`, `autowork`, `resync` e `ping` a cada 5 s |

Cuidados já no código: o token não aparece em log nem em corpo de resposta; a página não carrega nada de fora (Content-Security-Policy `default-src 'self'`); o aviso da CLI "Warning: claude.ai MCP servers blocked" é filtrado no servidor e não chega ao celular; desligar o servidor na aba Remoto derruba os WebSockets abertos (antes eles continuavam recebendo eventos); a web app reconecta sozinha, avisa "Conexão perdida" e recarrega os dados ao voltar.

Limites conhecidos:

- **Instalação como aplicativo:** por `http://` na rede local o navegador só oferece "Adicionar à tela inicial" (atalho). A instalação completa de PWA e o service worker exigem HTTPS; não há service worker.
- **Anexos:** aparecem como chips com o nome do arquivo; a miniatura das imagens não é enviada ao celular, e o celular não anexa arquivos.
- **Fila de mensagens:** não existe no remoto; com o agente trabalhando, a caixa fica desabilitada e o botão vira Parar.
- **Negrito de novidade:** vale para o que chega com a web app aberta (fica guardado no aparelho); o servidor não guarda o que cada aparelho já viu.
- **"Abrir" de um servidor** usa o IP do computador; servidor que só escuta em `localhost` não abre pelo celular.
- **Iniciar o auto-work numa sessão** não tem botão na web app (a rota `start` existe); ele começa pelas regras do desktop.

### Sobre `crates/app` (Dioxus)

A pasta ficou **fora do workspace Cargo** (`exclude = ["crates/app"]`) e serve só de referência de comportamento. Ela não compila mais sozinha. **Pode ser apagada depois que você conferir a interface nova na janela** e confirmar que nada da antiga faz falta.

## Funcionalidades

Legenda: **na janela** = visto funcionando na janela do Tauri; **compila** = implementado, passa em `tsc`/build, nunca exercitado na janela; **core testado** = a lógica em Rust tem teste, a tela não foi exercitada.

| Recurso | Estado na interface nova |
|---|---|
| Janela, barra de projetos e sessões, painel direito, barra superior, status bar | ✓ na janela (layout novo, de 1100 px para cima) |
| Temas Oceano, Nebulosa e Brasa (interface, Monaco e terminal) | ✓ na janela (interface e Monaco); terminal com tema não conferido |
| Explorador: árvore por projeto, status git por arquivo (M/U), abrir arquivo | ✓ na janela |
| Editor Monaco com cores (.rs, .toml, .md), abas, split, Ctrl+Tab/W/\\, Ctrl+S e indicador de modificado | ✓ na janela; **arrastar abas não conferido** |
| Terminais embutidos (ConPTY + xterm) | ✓ na janela (um terminal por vez; várias abas e mover entre grupos não conferidos) |
| Provedores claude/opencode e status | ✓ na janela ("conectado" nos dois); login pela CLI num terminal não conferido |
| Tarefas em worktree isolado, stream, diff, responder (`--resume`) | ✓ na janela com uma tarefa real do claude; **aplicar/descartar não conferidos** |
| Histórico SQLite persistente | ✓ na janela (a tarefa e o chat voltam depois de fechar e abrir) |
| Subagentes: árvore, cartão "→ subagente", chat somente leitura sem caixa de resposta | ✓ na janela com um subagente real do claude |
| Subagentes do opencode e subagente em segundo plano do claude | ⚠ escritos sem conferir contra saída real |
| Métricas de tokens e custo equivalente em API, gráfico por dia | ✓ na janela (atualiza ao fim do turno) |
| Dependências: detecção, confirmação antes de instalar, comando inexistente no terminal | ✓ na janela (só o pedido; nenhuma instalação foi executada) |
| Acesso remoto pela LAN (QR + token) | ✓ na janela: liga, QR, todas as rotas e o WebSocket sem token = 401, desliga, fecha a porta e derruba quem estava conectado |
| Web app do celular (refeita: projetos e sessões, conversa, subagentes, nova conversa, auto-work, servidores, temas) | ✓ num navegador emulando celular (390, 360 e 430 px, toque), contra o servidor real: lista, histórico da sessão #1, subagente sem caixa de resposta, `/context` enviado com a resposta chegando pelo WebSocket, reconexão, três temas, Derrubar num servidor real. ⚠ auto-work: ligar e desligar conferidos de verdade; progresso, Pausar, Retomar e Parar só com uma execução simulada na tela. ⚠ "Enviar" de Nova conversa não foi tocado (gastaria tokens). **Nunca aberta num celular de verdade** |
| Auto-work (PENDENCIAS.md, TODO/FIXME, build/testes, travas) | ⚠ core testado; o formulário aparece, nenhum ciclo rodou |
| Player de música (mídia do Windows + arquivos locais) | ⚠ aparece na status bar ("Nada tocando"); tocar mídia não conferido |
| Pet de uso | ✓ na janela (dormindo e trabalhando, balão com os números) |
| Servidores + painel lateral | ✓ na janela com `python -m http.server 8799` num terminal da IDE: aparece, "Derrubar" mata o processo e fecha a porta; vínculo a uma sessão de agente não conferido |
| Conversa (aba única: conversa nova + sessão) | ✓ na janela com a sessão gravada (histórico, passos, subagente, painel direito); **o primeiro envio criando a tarefa na mesma aba não foi conferido** (exige rodar um agente) |

O pedido de instalação por comando inexistente só aparece para ferramentas conhecidas (tabela em `crates/core/src/deps.rs`, por exemplo `gradle`, `pnpm`, `go`); um comando qualquer não abre nada.

## O que veio do chat do Digitama

Tudo em `src/views/chat`, com os comandos Rust em `src-tauri/src/commands/extra_chat.rs`. A aba de conversa (`src/views/session`) usa esses componentes. Os estados abaixo são de antes do redesenho; a caixa foi reorganizada (botões e pílulas) e não foi reexercitada com um agente rodando.

| Item | O que faz | Estado |
|---|---|---|
| 1.1 Markdown | títulos, listas, tabelas, código com rótulo da linguagem, copiar código e copiar mensagem | ✓ na janela (título, lista, código com rótulo); os botões de copiar aparecem, o clique não foi conferido |
| 1.2 Fila de mensagens | com o agente ocupado, Enter põe na fila; a fila sai quando o turno termina; Ctrl+Enter para o turno e envia | ✓ na janela (Enter e saída da fila); Ctrl+Enter não conferido |
| 1.3 Menções | `@arquivo` (busca no projeto) e `@agente` (arquivos de `.claude/agents` e `.opencode/agents`) | ✓ na janela |
| 1.4 e 1.5 Anexos | clipe, arrastar e soltar, colar print, texto colado grande vira anexo; miniaturas na mensagem | ✓ na janela: clipe (.txt e .png), texto grande colado e miniatura; arrastar e colar print não conferidos |
| 1.6 Comandos de barra | `/nova`, `/parar`, `/diff`, `/aplicar`, `/descartar`, `/sessoes`, `/ajuda` e os comandos da CLI do claude | ✓ na janela: menu com os do Argus e os da CLI, `/ajuda`, `/diff`; `/modelo` responde "não disponível aqui"; `/nova` abre uma conversa nova e `/diff` abre a aba Alterações do painel direito (não reconferidos depois do redesenho) |
| 1.7 Chamada de ferramenta escrita como texto | aviso com o botão "Executar de verdade" | ⚠ compila; detecção testada por script |
| 1.8 Uso do dia | pílula "hoje: X tokens · ≈ US$ Y (equivalente em API)" | ⚠ compila |
| 2.4 (atalho) Detecção de loop | cancela o turno quando o agente repete a mesma chamada; só vale com a conversa aberta numa aba | ⚠ compila |

Ficou pendente do Digitama:

- **1.9 Conexões e cartão do repositório:** não feito (é o passo 9 abaixo).
- **Imagens anexadas com o claude:** os anexos ficam em `%APPDATA%\argus\anexos\<tarefa>\`, fora da pasta de trabalho do agente. O `claude -p` pode negar a leitura. A saída provável é passar `--add-dir` em `task_command` (core). Não testado.
- **Comandos de barra da CLI:** na primeira vez que você digita `/` numa pasta, o Argus sobe um `claude -p --no-session-persistence`, manda `/context`, lê a linha de inicialização e mata o processo. Conferido contra a CLI real (claude 2.1.220), deixando o comando terminar: a resposta vem da própria CLI, com 0 tokens e custo 0. Para desligar mesmo assim, tire `chat_agent_capabilities` do `invoke_handler` em `src-tauri/src/main.rs`.
- **Comando de barra que o Argus não conhece** (por exemplo `/context`) vai para a CLI como mensagem. Os que a CLI resolve sozinha custam 0 tokens, mas contam como um turno da tarefa.
- **"Turnos" nas métricas** conta uma linha por modelo: um turno em que o subagente usa outro modelo aparece como 2.
- **`/modelo`:** aparece no menu e responde "ainda não está disponível" (depende da troca de modelo, item 2.2).
- **`@agente`** vai como texto; os agentes embutidos de cada CLI não entram na lista.
- **Fila e anexos** vivem só na memória da janela: não sobrevivem a reinício nem aparecem no remoto.
- **Realce de sintaxe** nos blocos de código do chat.
- **Detecção de loop no core** (item 2.4 definitivo), para valer com a aba fechada e no remoto.
- **Ctrl+C no Explorer + Ctrl+V** anexando pelo caminho real.
- **Licença:** a lógica de menções e fila veio da versão reescrita do Digitama, que cita um projeto MIT de terceiros. Conferir antes de distribuir.

## Pendências de validação

Implementado, mas ainda não visto funcionando na janela nova. Marque com `[x]` quando conferir.

- [ ] Primeiro envio numa conversa nova: criar a tarefa e a mesma aba virar a sessão (com stream ao vivo, botão Parar e fila).
- [ ] "N agentes rodando" na status bar e o ponto pulsando na barra esquerda com um agente de verdade (o negrito de "não lida" foi conferido com um evento simulado).
- [ ] Pílula de auto-work ligando e desligando o projeto; seta para cima recuperando o pedido anterior.
- [ ] Arrastar um arquivo da aba Arquivos para a caixa de composição (vira `@caminho`).
- [ ] Terminal com as cores do tema.
- [ ] Arrastar abas entre grupos, arrastar arquivo do Explorer e colar print.
- [ ] Aplicar e descartar uma tarefa com alterações.
- [ ] Auto-work rodando um ciclo na interface nova.
- [ ] Player tocando música.
- [ ] Login pela CLI num terminal.
- [ ] Tarefa pelo OpenCode.
- [ ] Web app aberta num celular de verdade.
- [ ] Servidor vinculado a uma sessão de agente.

## Defeitos conhecidos

- [ ] "Enviar" em Nova tarefa não abria a aba da sessão. O formulário deixou de existir: a conversa nova cria a tarefa e a mesma aba vira a sessão. Corrigido no código, **ainda não visto funcionando** (exige rodar um agente).
- [ ] As métricas contam 2 turnos quando um turno usa dois modelos.
- [x] O botão "Fazer login" aparecia mesmo com o provedor já conectado. Agora, conectado, aparece só "Trocar de conta" (visto na aba Provedores).
- [ ] Imagens anexadas podem não ser lidas pelo claude, porque ficam fora da pasta do projeto (provável saída: `--add-dir`).
- [x] O aviso da CLI do claude sobre servidores MCP bloqueados mostrava o nome da organização do usuário no chat. As linhas que começam com "Warning: claude.ai MCP servers blocked" não são mais renderizadas (`isHiddenNotice` em `views/session/events.ts`); visto com a sessão gravada, que tem 4 dessas linhas. O texto continua gravado no banco; no acesso remoto ele é filtrado no servidor e não chega ao celular.
- [ ] Um comando de barra que o Argus não conhece vai para a CLI e conta como turno.

## Próximos passos (em ordem)

1. **Terminar o teste na janela.** O que ainda não foi conferido, em ordem de risco:
   - arrastar aba dentro do WebView2 e soltar na borda para criar grupo;
   - terminal em aba oculta ganhando tamanho ao aparecer; mover aba de terminal entre grupos sem matar o processo;
   - login pela CLI num terminal; aplicar e descartar uma tarefa com alterações;
   - conversa: primeiro envio criando a tarefa na mesma aba, colar print, arrastar arquivo, Ctrl+Enter com o turno rodando;
   - player tocando mídia; auto-work rodando um ciclo.
2. **Servidores vinculados a uma sessão.** O painel já foi conferido com um servidor aberto num terminal da IDE. Falta o vínculo a uma sessão de agente, liberando `python -m http.server` em "Comandos permitidos".
3. **Abrir a web app num celular de verdade.** Na primeira vez, o Windows pede permissão no Firewall: clique em **Permitir** (rede privada). A web app foi refeita e conferida num navegador emulando celular; falta o aparelho real (teclado de verdade, área segura, "Adicionar à tela inicial", toque). Conferir também o auto-work com uma execução real e o primeiro envio de Nova conversa.
4. **Subagentes: conferir o que foi suposto.** Uma saída real do opencode com a ferramenta `task`, e um subagente em segundo plano do claude.
5. **Pequenos ajustes conhecidos:**
   - O botão "Reiniciar" de um servidor roda sem saída visível. Seria melhor reabrir o processo num terminal da IDE.
   - Janela separada de servidores não foi portada (a permissão para janelas `aux-*` já existe).
   - "Hoje" nas métricas vira às 21h de Brasília, porque os dias vêm em UTC do backend.
   - Trocar o `netstat` por GetExtendedTcpTable, o que exige adicionar a crate `windows` ao core.
   - Espaçar as verificações de build e testes do auto-work em projetos grandes, ou torná-las opcionais.
   - O OpenCode não informa o modelo usado; o custo cai na tabela de fallback.
   - Web app do celular: HTTPS (para instalar como PWA de verdade), miniaturas dos anexos e fila de mensagens.
   - A fila de mensagens e o rascunho da "conversa nova" são compartilhados entre todas as abas de conversa nova (chave `new`).
   - `OrchestratorEvent` não deriva `Serialize`; por isso existe o espelho `OrchMsg` em `src-tauri/src/events.rs`. Derivar no core elimina o espelho.
   - TypeScript/JavaScript no Monaco só valida sintaxe (a validação semântica foi desligada).
6. **Empacotamento:** `npm run tauri build` gera o instalador. O Tauri avisa que o identificador `dev.argus.app` termina em `.app`, o que conflita com bundles do macOS; no Windows não afeta.
7. **Apagar `crates/app`** depois de conferir a interface nova.
8. **Uso de CPU e GPU por sessão.**
   - Mostrar, em cada sessão, o consumo de CPU, memória e GPU dos processos dela (agente, subprocessos e servidores vinculados).
   - A árvore de processos por sessão já existe em `crates/core/src/procs.rs`; a CPU e a memória vêm do `sysinfo`.
   - GPU por processo no Windows: contadores de desempenho "GPU Engine" (PDH) ou `nvidia-smi` quando houver placa NVIDIA.
9. **Conexões com serviços de desenvolvimento.**
   - GitHub, Google Cloud, Firebase, Vercel e outros.
   - Seguir a mesma regra dos agentes: usar a CLI oficial de cada serviço (`gh`, `gcloud`, `firebase`, `vercel`), que faz o próprio login. A IDE não lê nem guarda tokens.
   - Tela "Conexões": detectar a CLI, mostrar o status de login, botão "Fazer login" no terminal embutido e ações por projeto (PRs e issues, deploys, logs).
   - Inclui o cartão do repositório do chat (item 1.9 do Digitama).
10. **Mapa de agentes (Agent map).**
    - Janela ou painel "Mapa de agentes" por sessão: à esquerda o nó da sessão principal (nome, modelo, tokens no contexto) e à direita, ligados por linhas, os subagentes que ela chamou.
    - Cada nó de subagente mostra o nome, um ponto de status (verde = rodando, cinza = concluído), a duração e os tokens gastos.
    - Cabeçalho com a contagem ("6 agentes · clique em um agente para ver detalhes"). Clicar num nó abre o chat somente leitura daquele subagente.
    - Embaixo, a lista de tarefas em segundo plano da sessão (comandos de shell em execução, com o tempo), clicável para ver a saída.
    - Usa os eventos `SubagentStart`/`SubagentEnd` do core (já prontos) e a árvore de processos de `procs.rs`.
11. **Janela de contexto e % de uso.**
    - Mostrar, em cada sessão, quanto da janela de contexto do modelo está ocupada: uma linha (barra fina) com a porcentagem, no cabeçalho do chat ou junto da caixa de composição.
    - Passar o mouse sobre a linha abre o detalhe dos tokens em contexto: entrada, saída, cache lido, cache gravado, total e o tamanho da janela do modelo; se a CLI informar, a divisão por categoria (prompt de sistema, ferramentas, mensagens, arquivos).
    - Fontes: o campo `usage` de cada turno no stream-json e o tamanho da janela por modelo; para o claude, o comando `/context` devolve a divisão por categoria com custo 0.
    - Avisar quando passar de um limite (por exemplo 80%).

## Notas

- Os worktrees dos agentes ficam em `<projeto>/../.argus-worktrees/`. Worktrees antigos em `.harness-worktrees` podem ser apagados com `git worktree prune` no projeto de origem.
- A pasta do projeto ainda se chama `Harness`. Nenhum caminho absoluto está no código; ao renomear, `target/` recompila do zero.
- O `index.html` não pode ter `<style>` inline: o `vite build` falha quando o diretório atual vem com a letra do drive minúscula (`d:\`).
- Nada da migração foi commitado ainda.

## Decisões fixas

- Os agentes são as CLIs oficiais, com o login da assinatura. Nada de chave de API, e a IDE não lê tokens.
- O claude roda com `--permission-mode acceptEdits`. O Bash fica bloqueado, a não ser o que estiver liberado em "Comandos permitidos" por projeto.
- Nenhuma marca ou nome de empresa no projeto. O identificador do app é `dev.argus.app`.
- O custo é sempre exibido como "equivalente em API".
- Textos da interface em português do Brasil. Windows primeiro.
