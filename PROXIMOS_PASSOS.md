# Argus — Estado atual e próximos passos

_Atualizado em 2026-11-16. Base: 2026-11-14, depois da migração da interface de Dioxus para Tauri 2, do redesenho do layout (chat no centro, temas) e de uma rodada de validação na janela com tarefas reais._

O Argus é uma IDE desktop que gerencia vários projetos de uma pasta raiz e orquestra agentes de IA (Claude Code, OpenCode, Codex e Gemini). A lógica fica em Rust (`argus-core`); a interface é React + TypeScript dentro de uma janela Tauri 2. Os agentes usam o login das próprias CLIs oficiais; a IDE nunca lê nem guarda tokens.

**Estado em uma frase:** tudo compila, os testes passam e quase tudo foi exercitado na janela em 2026-11-14 com tarefas reais do claude e do opencode (conversa nova, fila, anexos, diff, aplicar, descartar, auto-work, servidores, terminais, arrasto de abas). A coluna "Estado" da tabela abaixo diz o que foi conferido de verdade e o que ainda falta.

A conferência na janela foi feita pelo protocolo de depuração do WebView2 (cliques e teclas enviados à página, sem usar o mouse real). Para repetir: abra o executável com `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9333` e conecte em `http://127.0.0.1:9333`.

- **Arrastar** (abas, arquivo da aba Arquivos): o arrasto começa de verdade na página (botão pressionado e movido dispara o `dragstart`, com o `DataTransfer` real) e é interceptado no ponto em que o navegador o entregaria ao sistema (`Input.setInterceptDrags`); os `dragenter`, `dragover` e `drop` seguintes entram por `Input.dispatchDragEvent`. Limite: o laço de arrasto do Windows não roda, então um arquivo vindo do Explorer do Windows não é coberto.
- **Colar imagem:** evento `paste` sintético com um PNG no `DataTransfer`. Limite: a área de transferência real do sistema (Ctrl+V depois de um print) não é coberta.
- **Caixas nativas de confirmação** (Aplicar, Descartar, "Parar e enviar"): a página fica parada até a resposta; o teste respondeu mandando a mensagem de clique ao botão "Sim" da caixa.

## Pendências por prioridade (2026-11-19)

Fila de trabalho. P1 a P3 em andamento; P4 é conferência que o usuário faz em casa (aparelho, sistema, chave).

### P1 — uso e custo do dia a dia
- [ ] Números em pt-BR na conversa: total e tooltip (`session/index.tsx`), `Transcript.tsx`, `ContextMeter.tsx`.
- [ ] `cd <pasta>; git status` negado no PowerShell: conferir que `cd`/`Set-Location` encadeado passa.
- [ ] Aviso do modo Isolado não impede o agente de procurar o que não existe na cópia.
- [ ] OpenCode: regras "ask" sem quem aprove no `run`; edição de arquivos no v2.
- [ ] Worktree preso por processo do OpenCode ao Aplicar/Descartar.
- [ ] Turno cancelado registra uso (conferir com cancelamento real).

### P2 — funções novas sem conferência real
- [ ] Tarefas agendadas: recorrências, "Executar agora", toast, horário perdido (o boot do Windows fica no P4).
- [ ] Planejar/Bypass num turno real: "Executar o plano", selos na aba e na barra de status.
- [ ] Extensão VS Code: Alt+K com seleção, chip de erros, rejeitar trecho em CRLF, "Liberar e repetir".
- [ ] Inspetor › Pendências: atualização ao vivo, destaque no turno, quatro abas em 340 px.
- [ ] Auto-work: Pausar, Retomar, Parar, travas, verificação de build/testes.
- [ ] Codex e gemini: `--add-dir`/`--include-directories` na pasta `_Argus` (CLIs não instaladas aqui).

### P3 — melhorias
- [ ] Checkpoints por turno ("Voltar a este ponto").
- [ ] Alt+K sem conversa aberta.
- [ ] Aviso de custo de cache na pílula de modelo (1ª tarefa haiku, troca de modelo).
- [ ] Servidor aberto pelo agente morre no fim do turno.
- [ ] Página de Aparência com espaço vazio; ícones por item na paleta Ctrl+K.
- [ ] Laços de "rodando": deixar o anel como único foco.

### P4 — o usuário confere em casa
- [ ] Celular: pareamento pelo QR (cookie `Lax`), fonte Geist, divisória de troca de agente / comando negado / subtítulo na web app.
- [ ] Métricas: virada do "Hoje" depois das 21h.
- [ ] OpenCode real: modelo `-free` com custo 0, uso do último passo.
- [ ] Selo de auto-work sem "Reiniciar" em sessão aplicada/descartada.
- [ ] Arrastar do Explorer, Ctrl+V de print, login do claude no terminal.
- [ ] Player tocando música.
- [ ] "Iniciar com o Windows" no app instalado (reiniciar e ver abrir minimizado).
- [ ] APK assinado (chave do usuário).

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
cargo test --workspace      # 141 testes do core + 18 do desktop
node --check crates/core/assets/remote/app.js   # sintaxe da web app do celular
```

A interface ainda não tem runner de testes próprio; a lógica pura das telas foi conferida por scripts avulsos durante a migração.

Variáveis úteis para testes:

| Variável | O que faz |
|---|---|
| `ARGUS_ROOT` | abre essa pasta raiz automaticamente |
| `ARGUS_DB` | usa outro banco SQLite |
| `ARGUS_ALLOWED_TOOLS` | usa outro arquivo de comandos permitidos |
| `ARGUS_OPENCODE_MODEL` | só o modelo INICIAL das conversas novas do OpenCode (`provedor/modelo`, como em `opencode models`); o de cada conversa é escolhido na pílula de modelo e fica gravado na tarefa |

Os dados ficam em `%APPDATA%\argus\`: `argus.db`, `pricing.json`, `allowed_tools.json` e a pasta `anexos\` (anexos do chat, apagados depois de 1 dia).

## Estrutura

```
package.json, vite.config.ts, index.html     front-end (vite)
src/                                         interface em React 18 + TypeScript + zustand
  api/        uma função tipada por comando Tauri, eventos e tipos que espelham o serde
  store/      useApp, useLayout, useWorkbench, useTerminals, useTheme, bootstrap (liga eventos ao estado)
  shell/      Shell (layout), TopBar, RightPanel, BottomPanel, StatusBar, ContextMenu, icons (Lucide, mesma API Icon/IconName/toolIcon),
              useActiveContext (sessão e projeto da aba em foco), useGlobalShortcuts
  styles/     tokens.css (TODAS as cores por tema, mais fonte, espaçamento --space-1..6 e raios --radius-sm/--radius/--radius-lg/--radius-xl), components.css (classes portadas), shell.css (layout)
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
                 remote, media, system, extra_agents, extra_chat, schedules (tarefas agendadas,
                 notificação do sistema e "Iniciar com o Windows")
  src/events.rs  repassa os eventos do orquestrador e dos servidores para a interface
src-mobile/   crate argus-celular (Tauri 2 Android, dev.argus.celular): casca do aplicativo do
              celular. Só a tela de pareamento (web/); depois disso a webview abre a interface
              servida pelo computador. Fora do workspace; gerar com `gerar-apk.ps1` (ver README)
crates/core   (argus-core): workspace, monitor, pty, procs, deps, metrics, db, remote, autowork, schedules,
              isolation (o que a cópia isolada não teria), permissions (comandos liberados ao agente),
              agents/{provider, claude, opencode, codex, gemini, orchestrator, handoff (resumo na troca de agente)}
              assets/remote (web app do celular: index.html, app.js, style.css, unauthorized.html,
              manifest.webmanifest, icon.svg; embutidos no executável por include_str)
```

Monaco e xterm entram por npm e são empacotados pelo vite; nada vem de CDN.

## Layout

Inspirado na Orca (projetos e sessões à esquerda, abas no centro, arquivos à direita) e no chat do Claude Code no VS Code. Não há mais activity bar.

| Região | O que tem |
|---|---|
| Barra superior (32 px) | alternar a barra esquerda, projeto e sessão em foco, alternar terminal e painel direito |
| Barra esquerda | "Abrir pasta raiz", Painel, filtro de encerradas, recolher tudo e busca. Cada projeto (nome, branch, ±alterados) expande nas sessões: ponto de status, título (primeiro pedido), selos de auto-work e servidores, provedor. Sessão com atividade não vista fica em negrito. "+" no hover cria conversa no projeto; clique direito: Abrir, Ver diff, Cancelar o turno, Descartar, Excluir sessão (também no seletor de conversas do cabeçalho do chat); no projeto: Nova conversa e Excluir encerradas. A lista se atualiza sozinha (watcher do primeiro nível da raiz com debounce de 500 ms, foco da janela e o botão Atualizar); projeto apagado do disco com sessões fica como "pasta ausente". Excluir apaga tarefa e eventos, mas mantém as linhas de uso nas Métricas (órfãs, com o id original; ids nunca são reaproveitados). Rodapé: Provedores, Dependências, Métricas, Remoto e Configurações abrem como abas |
| Centro | grupos de abas com split (até 3). A aba de conversa é a principal: "Nova conversa" (saudação, sugestões, pílulas de projeto e provedor) vira a sessão da tarefa no primeiro envio, na mesma aba. Não existem mais as abas "Chat" e "Sessão" separadas |
| Conversa | cabeçalho com seletor de conversas (busca), "+", estado, "provedor · modelo" (some em aba estreita), total de tokens e custo equivalente em API. Coluna central de 820 px: pedido em caixa, resposta em Markdown, ferramentas em linhas recolhidas, "N passos" no turno terminado, divisória "Agente trocado: X → Y" e caixa "Comando negado" com o atalho "Liberar este comando neste projeto". Caixa de composição arredondada, numa linha só: anexar, "/", "@", pílula de projeto (só na conversa nova), pílula "provedor · modelo" (menu com agente e modelo; estreita, mostra só o modelo), botão de opções (ícone de ajustes, com contagem do que foge do padrão: modo de trabalho, modo de permissão e auto-work, cada um com explicação) e Enviar, que vira Parar durante o turno. Seta para cima recupera os pedidos anteriores |
| Pílulas da conversa | **Conversa nova:** projeto, provedor, modelo ("Padrão da CLI" + os que a CLI informa) e modo de trabalho ("Isolado (cópia do projeto)" ou "Direto na pasta"; sem git, só o direto). **Sessão existente:** o provedor pode ser trocado com o turno parado (desabilitado durante o turno, com dica), o modelo pode ser trocado a qualquer hora (vale no próximo turno) e o modo de trabalho é fixo. `/modelo` abre a lista; `/modelo nome` troca direto. A lista é pedida à CLI assim que a caixa aparece; enquanto não chega mostra "Carregando modelos…", em erro mostra a mensagem e "Tentar de novo" (erro e lista vazia nunca vão para o cache) e sempre dá para digitar o nome de um modelo |
| Aviso antes do envio | No modo Isolado, o primeiro envio compara a pasta do projeto com o que o worktree teria. Havendo algo de fora (não versionados, ignorados relevantes, alterações não commitadas), o pedido fica segurado num aviso acima da caixa, com o caminho citado em destaque e os botões "Usar Direto na pasta" e "Continuar isolado" (✕ devolve o texto à caixa). Em "Direto na pasta", avisa antes de iniciar uma segunda sessão direta no mesmo projeto |
| Painel direito (retrátil) | Arquivos (projeto ativo), Alterações (diff da sessão, Aplicar e Descartar; numa sessão "Direto na pasta", no lugar dos botões vem o aviso de que não há o que aplicar e o diff da pasta em relação ao último commit), Subagentes (árvore; abre o chat somente leitura), Servidores |
| Configurações | Tema; **Agentes** (modo de trabalho padrão, "Git local", "Git remoto" e a lista do que está liberado por padrão); **Por projeto** (as mesmas três escolhas, com "Seguir o geral", auto-work e comandos permitidos) |
| Painel inferior | terminais (oculto por padrão; abre pelo botão Terminal, Ctrl+J ou ao criar um terminal). "+" abre outro terminal no painel; "⧉" abre um terminal como aba do editor, que pode ser arrastada entre os grupos |
| Status bar | "N agentes rodando" (clique foca a sessão), servidores, uso de hoje; player, direção do split e pet |

Atalhos novos: Ctrl+N (nova conversa), Ctrl+B (barra esquerda), Ctrl+J (terminal); não valem com o foco num terminal.

"Projeto ativo" (`useApp.selected`) acompanha a aba em foco (`shell/useActiveContext.ts`) e pode ser trocado clicando num projeto na barra esquerda. É ele que a aba Arquivos, os terminais novos e as telas por projeto usam.

### Temas

Três temas, escolhidos em Configurações e lembrados no localStorage (`argus.tema`): **Oceano** (padrão), **Nebulosa** e **Brasa**. Toda cor vem de `src/styles/tokens.css`: cada tema define os tokens de base (`--bg`, `--surface`, `--surface-2`, `--border`, `--border-strong`, `--text`, `--text-2`, `--accent`, `--accent-hover`, `--on-accent`, `--accent-text`, `--selection`, `--focus`, `--status-bg`, `--on-status`) só com cores da própria paleta; os demais são derivados. As cores de estado (`--ok`, `--warn`, `--err`, `--run`) são iguais nos três. Nenhum componente usa hex direto. O Monaco (`views/workbench/monaco.ts`) e o xterm (`views/terminal/session.ts`) montam o tema a partir dos tokens e trocam junto.

Desde 2026-11-16 (fases 1 a 3 de `docs/PLANO_UI_ORCA.md`): fonte **Geist** variável (OFL-1.1) embutida pelo pacote npm `@fontsource-variable/geist`, sem CDN, em `--font-sans`; mono Cascadia em `--font-mono-family`. A hierarquia vem do tom das superfícies: `--border` é só ~7–10% do texto sobre a superfície (a `--border-strong` continua com a cor da paleta, nos controles e na caixa de composição) e a status bar é neutra (`--status-bg` = fundo do app, `--on-status` = texto secundário, com uma borda no topo). Ícones: `lucide-react` (ISC) com versão fixada; `src/shell/icons.tsx` mapeia os nomes antigos para os componentes Lucide (traço 1,75).

A web app do celular (`crates/core/assets/remote/style.css`) repete os mesmos tokens e os mesmos três temas; o seletor fica na tela Ajustes dela e a escolha é guardada no aparelho (mesma chave `argus.tema`). O `theme-color`, o manifest e o ícone acompanham o tema (`/manifest.webmanifest?tema=...`).

### Acesso remoto (web app do celular)

Servidor axum em `crates/core/src/remote.rs`, desligado por padrão; liga na aba Remoto, que mostra o QR code. A web app é HTML, CSS e JavaScript puros (sem framework e sem CDN) e foi refeita em 2026-11-14 para acompanhar o desktop.

| Tela | O que tem |
|---|---|
| Pareamento | `/pair?token=...` grava o cookie (HttpOnly, SameSite=Strict) e redireciona para `/`, tirando o token do endereço. Token ausente, errado ou revogado dá a mesma página 401 "Não autorizado" |
| Sessões (início) | projetos com as suas sessões: ponto de status (erro vira ícone de alerta com o rótulo "Erro"), título, provedor, selos de auto-work e de servidores, negrito e ponto azul na sessão com novidade, busca, "Encerradas", "+" geral e por projeto |
| Conversa | pedido em caixa, resposta em Markdown (tabela com rolagem lateral, código com rótulo e Copiar), ferramentas e "N passos" recolhidos, stream pelo WebSocket, total de tokens e custo equivalente em API, caixa de composição no rodapé (acompanha o teclado), Enviar que vira Parar. O subtítulo mostra projeto, provedor e modelo atuais (e "direto na pasta"); a divisória "Agente trocado: X → Y" e a linha "Comando negado" aparecem. Trocar de agente, de modelo e de modo de trabalho é só no desktop (conversa criada pelo celular usa os padrões do projeto) |
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

## Modelo, agente, modo de trabalho e permissões (2026-11-14)

**Modelo por conversa.** Fica na tarefa (coluna `model`). claude: `--model` em todo turno, junto com `--resume`; opencode: `-m`. A lista vem da própria CLI, sob demanda, com cache em memória: no claude, da resposta ao pedido de controle `initialize` do stream-json (`response.response.models`; nenhum token gasto, o campo de conta não é lido); no opencode, de `opencode models`. Nomes são validados antes de virar argumento (nada com espaço ou começando por `-`).

**Troca de agente.** `Orchestrator::set_provider`: a tarefa, a pasta e o histórico continuam; a sessão e o modelo do provedor que sai ficam em `provider_state` (JSON na tarefa) e voltam se o usuário retornar a ele (aí o `--resume`/`--session` é retomado e ele só recebe o que perdeu). O próximo turno leva na frente do pedido um resumo montado pelo Argus (`agents/handoff.rs`): pedidos do usuário, respostas em texto do agente principal e a lista de arquivos alterados (`git status` da pasta de trabalho), sem saída de ferramentas, até 24 mil caracteres (corta o meio e avisa). O histórico grava só a mensagem do usuário e o evento `ProviderSwitch` (a divisória). O uso continua na mesma tarefa, com provedor e modelo em cada linha.

**Modo de trabalho por tarefa** (coluna `mode`; o booleano global de worktrees saiu do `Orchestrator`). Isolado = git worktree, como antes. Direto = a pasta do projeto: sem Aplicar/Descartar, diff contra o último commit (sem tocar no índice do git). Padrão: Isolado; preferência geral e por projeto na tabela `settings` do banco. Pasta sem `.git` só aceita o direto. No Isolado o claude recebe `--append-system-prompt` dizendo que está numa cópia, onde fica a pasta original e que arquivos fora do git não existem ali (o opencode não tem essa opção: o aviso vai na frente do primeiro pedido). Nunca entra `--add-dir` da pasta original.

**Comandos liberados ao agente** (`crates/core/src/permissions.rs`). No Windows o claude usa a ferramenta `PowerShell`, não a `Bash`: por isso nem `git status` funcionava, e a lista do projeto (que virava só `Bash(...)`) não ajudava. Agora todo turno do claude leva `--allowedTools` com, para `Bash` e `PowerShell`:

- Git só leitura: `git status`, `git diff`, `git log`, `git show`, `git ls-files`, `git rev-parse`, `git blame`, `git grep` (com argumentos); `git branch` nas formas `git branch`, `--list`, `--show-current`, `-a`, `-r`, `-v`, `-vv`; `git remote` e `git remote -v`; `git stash list`.
- Inspeção: `cd`, `ls`, `dir`, `pwd`, `cat`, `type`, `head`, `tail`, `wc`, `which`, `where`; só no PowerShell: `Set-Location`, `Get-ChildItem`, `Get-Content`, `Get-Location`, `Get-Command`, `Test-Path`.
- Nível "Git local" (ligado por padrão no Isolado, desligado no Direto): `git add`, `commit`, `restore`, `checkout`, `switch`, `branch`, `merge`, `rebase`, `stash`, `tag`.
- Nível "Git remoto" (desligado por padrão nos dois): `git push`, `pull`, `fetch`, `clone`; com ele ligado, o push forçado segue negado por `--disallowedTools`.
- Nunca por padrão: `git push --force`, `git reset --hard`, `git clean`, shell genérico.

**Modo de permissão (2026-11-16)**: "Auto (recomendado)" é o padrão; "Restrito" é o comportamento acima. Geral e por projeto em Configurações (chave `perm_mode`). No Auto só os nocivos (`crates/core/src/harmful.rs`: apagar raiz/home/disco, formatar, desligar, registro/serviços/usuários, push forçado, reset --hard, clean, baixar e executar, credenciais, chmod/icacls amplos, matar processo por nome) ficam bloqueados: claude `--permission-mode auto` + nocivos em `--disallowedTools`; codex rede liberada no sandbox; gemini `yolo` + `--admin-policy` com `deny`; opencode `OPENCODE_CONFIG_CONTENT`. ⚠ Conferido só a montagem (testes) e o claude aceitando os argumentos (Haiku); gemini/codex não instalados aqui, opencode não rodado com a política.

A lista "Comandos permitidos ao agente" do projeto soma a isso (linha simples vira `Bash(...)`; para o PowerShell escreva `PowerShell(comando)`). Comando negado vira o evento `Denied` (lido de `permission_denials` do `result`) e aparece no chat com o atalho "Liberar este comando neste projeto". O OpenCode não foi alterado: as permissões dele vêm da configuração de agentes do próprio usuário.

## Funcionalidades

Legenda: **na janela** = visto funcionando na janela do Tauri; **compila** = implementado, passa em `tsc`/build, nunca exercitado na janela; **core testado** = a lógica em Rust tem teste, a tela não foi exercitada.

| Recurso | Estado na interface nova |
|---|---|
| Janela, barra de projetos e sessões, painel direito, barra superior, status bar | ✓ na janela (layout novo, de 1100 px para cima) |
| Temas Oceano, Nebulosa e Brasa (interface, Monaco e terminal) | ✓ na janela, inclusive o terminal: com um terminal aberto, o fundo e o texto do xterm trocam junto com o tema |
| Explorador: árvore por projeto, status git por arquivo (M/U), abrir arquivo | ✓ na janela |
| Editor Monaco com cores (.rs, .toml, .md), abas, split, Ctrl+Tab/W/\\, Ctrl+S e indicador de modificado | ✓ na janela, inclusive arrastar abas: para a borda (cria grupo), para a faixa de outro grupo e para cima de uma aba (entra antes dela) |
| Terminais embutidos (ConPTY + xterm) | ✓ na janela: várias abas no painel inferior, cada uma com o seu processo; terminal como aba do editor arrastado de um grupo para outro sem matar o processo |
| Provedores claude/opencode e status | ✓ na janela ("conectado" nos dois). "Trocar de conta" do OpenCode abre o `opencode auth login` num terminal (visto; fechado sem concluir). O do claude não foi aberto (o login dele abre o navegador) |
| Modelo por conversa (pílula, `/modelo`, cabeçalho, banco) | ✓ na janela com tarefas reais: lista do claude (opus[1m], claude-fable-5[1m], sonnet, haiku) e do opencode (146, com busca); sessão trocada de haiku para sonnet e a linha de uso seguinte veio como `claude-sonnet-5`. ⚠ a troca de modelo no claude regrava o cache inteiro da conversa (visto: 107 mil tokens de cache gravados, US$ 0,64 equivalentes em API num turno de uma linha) |
| Troca de agente na sessão (Claude Code ↔ OpenCode) | ✓ na janela com tarefa real: claude → OpenCode, divisória no chat, sessão nova do opencode respondeu certo qual tinha sido o primeiro pedido e o arquivo alterado (veio do resumo). Voltar ao provedor anterior retomando a sessão guardada: só teste automático |
| Modo de trabalho (Isolado / Direto na pasta) e aviso antes do envio | ✓ na janela com tarefas reais: pasta fora do git + Isolado mostra o aviso antes de enviar, com o caminho citado em destaque; "Continuar isolado" roda no worktree e o agente responde em 2 passos que a pasta não existe na cópia; "Usar Direto na pasta" roda na pasta, acha e edita o arquivo, e a aba Alterações mostra o diff sem Aplicar/Descartar. ⚠ o aviso de segunda sessão direta e as preferências em Configurações gravando: lógica com teste automático, não exercitados com duas sessões reais |
| Git e inspeção liberados por padrão; "Comando negado" no chat | ✓ na janela com tarefa real (claude): `git status` e `git log -1` rodaram, `git push` foi negado e apareceu a caixa "Comando negado" com o atalho. ⚠ níveis "Git local"/"Git remoto" ligados e o clique no atalho: montagem com teste automático, não vistos numa tarefa real. ⚠ OpenCode: não rodou o `git status` pedido no teste (terminou o turno sem chamar ferramenta); permissões dele não foram alteradas |
| Tarefas em worktree isolado, stream, diff, responder (`--resume`) | ✓ na janela com tarefas reais do claude: Aplicar leva a alteração ao projeto (commit, merge, worktree e branch removidos) e Descartar remove o worktree e a branch. Aplicar e descartar uma sessão do OpenCode não foram conferidos |
| Tarefa pelo OpenCode | ✓ na janela com `ARGUS_OPENCODE_MODEL=openai/gpt-6-luna`: stream, ferramenta, tokens e custo equivalente. ⚠ o opencode 2.0.22 não imprime o `step_finish` do último passo, então o uso desse passo não entra; edição de arquivos pelo OpenCode não conferida |
| Histórico SQLite persistente | ✓ na janela (a tarefa e o chat voltam depois de fechar e abrir) |
| Subagentes: árvore, cartão "→ subagente", chat somente leitura sem caixa de resposta | ✓ na janela com um subagente real do claude |
| Subagentes do opencode e subagente em segundo plano do claude | ⚠ escritos sem conferir contra saída real. A linha real de ferramenta do opencode 2.0.22 foi conferida e virou teste (o id da chamada vem em `part.id`), mas nenhuma execução trouxe a ferramenta `task` |
| Métricas de tokens e custo equivalente em API, gráfico por dia | ✓ na janela (atualiza ao fim do turno). "Turnos" conta turnos, não linhas de uso (visto: 8 turnos com 11 linhas). Os dias seguem o fuso do computador (teste automático com fuso fixo; a virada do dia não foi vista na janela) |
| Dependências: detecção, confirmação antes de instalar, comando inexistente no terminal | ✓ na janela (só o pedido; nenhuma instalação foi executada) |
| Acesso remoto pela LAN (QR + token) | ✓ na janela: liga, QR, todas as rotas e o WebSocket sem token = 401, desliga, fecha a porta e derruba quem estava conectado |
| Web app do celular (refeita: projetos e sessões, conversa, subagentes, nova conversa, auto-work, servidores, temas) | ✓ num navegador emulando celular (390, 360 e 430 px, toque), contra o servidor real: lista, histórico da sessão #1, subagente sem caixa de resposta, `/context` enviado com a resposta chegando pelo WebSocket, reconexão, três temas, Derrubar num servidor real. ⚠ auto-work: ligar e desligar conferidos de verdade; progresso, Pausar, Retomar e Parar só com uma execução simulada na tela. ⚠ "Enviar" de Nova conversa não foi tocado (gastaria tokens). **Nunca aberta num celular de verdade** |
| Auto-work (PENDENCIAS.md, TODO/FIXME, build/testes, travas) | ✓ na janela: um ciclo real com 2 itens, da pílula ligada até "auto concluído 2/2", com o selo, a barra de progresso e a etiqueta "auto" na barra esquerda. Pausar, Retomar, Parar, as travas e a verificação de build/testes não foram exercitados na janela |
| Player de música (mídia do Windows + arquivos locais) | ⚠ a mídia do sistema é detectada (título e autor do que estava tocando apareceram e mudaram sozinhos na status bar); os controles e os arquivos locais não foram conferidos |
| Pet de uso | ✓ na janela (dormindo e trabalhando, balão com os números) |
| Servidores + painel lateral | ✓ na janela com um terminal da IDE e com uma sessão de agente: com `python -m http.server:*` em Comandos permitidos, o servidor aberto pelo claude aparece no selo da sessão, na status bar e na aba Servidores ("Sessão #N"); "Derrubar" mata o processo e fecha a porta. Limite: o `claude -p` encerra os processos em segundo plano quando o turno termina, então o servidor só vive durante o turno |
| Conversa (aba única: conversa nova + sessão) | ✓ na janela com tarefas reais: o primeiro envio cria a tarefa na mesma aba, com stream ao vivo, Enviar virando Parar, fila, "1 agente rodando", ponto pulsando e negrito quando a novidade chega com a aba fora de foco |
| Caixa de composição numa linha (pílula "provedor · modelo", opções, pílula de projeto) | ✓ na janela em 2026-11-16: lista de modelos do claude (Padrão, opus[1m], claude-fable-5[1m], sonnet, haiku) e do opencode (busca, "Usar “nome”" para modelo digitado); botão de opções com explicações, Esc fecha e devolve o foco, contagem "1" com Direto na pasta num projeto git; pílula de projeto só na conversa nova. Corrigido na mesma rodada: a pílula de agente e modelo era cortada ("Claude Code · pad…") e, em aba estreita (split), a barra quebrava em duas linhas. "Carregando modelos…" não foi visto: a lista já estava pronta quando a pílula foi aberta |
| Configurações com navegação lateral (Tema, Agentes, Provedores, Por projeto) | ✓ na janela em 2026-11-16: os três temas trocam a interface inteira; Provedores mostra claude e opencode "conectado"; o rodapé não tem mais Provedores. Corrigido: num grupo estreito (split) a navegação não empilhava e os campos estouravam o cartão (agora pela largura do grupo) |
| Pasta raiz que se atualiza sozinha | ✓ na janela em 2026-11-16: criar, renomear e apagar uma subpasta de projeto aparece na lista em 440–565 ms; botão Atualizar redetecta também os comandos de build; projeto apagado com sessão vira "pasta ausente" (riscado) e some depois de excluir a última sessão |
| Excluir sessão e "Excluir encerradas" | ✓ na janela em 2026-11-16: pelo menu da barra esquerda e pelo clique direito no seletor de conversas do cabeçalho (caixa nativa com o texto certo para modo Direto), aba da sessão fecha; "Excluir encerradas (1)" depois de descartar uma sessão isolada do OpenCode (worktree e branch removidos). Métricas iguais antes e depois (US$ 0,18 → 0,18; no fim, US$ 0,79 com 5 turnos, incluindo os das sessões excluídas) |
| Janela de contexto (% no cabeçalho) | ✓ na janela em 2026-11-16 com haiku: 19% e dica "38.297 de 200.000 tokens"; opus[1m]: 4% de 1 M; OpenCode sem tamanho de janela conhecido mostra só os tokens |
| Mapa de agentes | ✓ na janela em 2026-11-16 com um subagente real (Task, haiku): aberto pelo cabeçalho e por "Abrir mapa" da aba Subagentes; dois nós ligados por linha, "2 agentes"; Enter no nó do subagente abre o chat somente leitura (prompt recebido, resposta, resultado devolvido, sem caixa de resposta) |
| Builds (tela no rodapé) | ✓ na janela em 2026-11-16: comandos detectados de um projeto Node, de um Cargo e de uma cópia dos manifestos do Argus (app:dev :1420, dev :5173, preview :4173, app:instalador, build, cargo build/--release); `npm run build` trivial: histórico "concluído · 1s · botão", artefato `dist 1 B` com Abrir pasta, aviso "Build concluído em …" na status bar. Builds pesados do Argus não foram rodados |
| Servidor de dev com um clique | ✓ na janela em 2026-11-16 (script `vite` falso): ▶ na tela Builds e no hover do projeto, "pronto" com a porta respondendo, Abrir no navegador, Parar fecha a porta; com a 5173 ocupada, "A porta 5173 está ocupada; usando a 5174." e o servidor sobe na 5174. Corrigido: o botão do comando mostrava ":5173 (rodando)" com o servidor na 5174 |
| Anexos por aba de conversa nova | ✓ na janela em 2026-11-16: duas conversas novas (uma por grupo do split) gravam em `anexos\novo\<id da aba>\` diferentes. Observação: o "+" da barra esquerda e o "+" da faixa de abas reaproveitam a aba de conversa nova já aberta no grupo |
| Inspetor › Pendências (2026-11-19): check-list do PROXIMOS_PASSOS.md, PENDENCIAS.md ou TODO.md da raiz (do worktree da sessão, se houver), feito/total e barra por seção, tarefas TodoWrite do agente no turno com o `in_progress` destacado, item que o agente editou no arquivo destacado, quadro "Trabalhando agora" no topo, clique num item aberto põe "Trabalhe na pendência: …" na caixa | compila + core testado (parser em `crates/core/src/checklist.rs`, comando `project_checklist`; tela em `src/views/session/ChecklistPanel.tsx` e `checklist.ts`). Falta ver na janela: atualização ao vivo pelo watcher, destaque durante um turno real e quatro abas cabendo em 340 px |
| Passada rápida (2026-11-16) | ✓ terminal (eco no PowerShell), explorador e editor Monaco, Métricas, Remoto (liga na 8787, QR, `/` sem token = 401, desliga e a porta fecha), Dependências ("Tudo em ordem"). Nenhum erro nem exceção no console ao recarregar a página e percorrer as telas. Corrigido: com muitos terminais, a faixa de abas do painel inferior vazava por cima do painel direito |

O pedido de instalação por comando inexistente só aparece para ferramentas conhecidas (tabela em `crates/core/src/deps.rs`, por exemplo `gradle`, `pnpm`, `go`); um comando qualquer não abre nada.

## O que veio do chat do Digitama

Tudo em `src/views/chat`, com os comandos Rust em `src-tauri/src/commands/extra_chat.rs`. A aba de conversa (`src/views/session`) usa esses componentes. Os estados abaixo são de antes do redesenho; a caixa foi reorganizada (botões e pílulas) e não foi reexercitada com um agente rodando.

| Item | O que faz | Estado |
|---|---|---|
| 1.1 Markdown | títulos, listas, tabelas, código com rótulo da linguagem, copiar código e copiar mensagem | ✓ na janela (título, lista, código com rótulo); os botões de copiar aparecem, o clique não foi conferido |
| 1.2 Fila de mensagens | com o agente ocupado, Enter põe na fila; a fila sai quando o turno termina; Ctrl+Enter para o turno e envia | ✓ na janela com agente real: Enter, saída da fila e Ctrl+Enter (pergunta, para o turno e manda a mensagem) |
| 1.3 Menções | `@arquivo` (busca no projeto) e `@agente` (arquivos de `.claude/agents` e `.opencode/agents`) | ✓ na janela |
| 1.4 e 1.5 Anexos | clipe, arrastar e soltar, colar print, texto colado grande vira anexo; miniaturas na mensagem | ✓ na janela: clipe (.txt e .png), texto grande colado, miniatura, imagem colada (evento sintético) e lida pelo claude numa tarefa real, arquivo arrastado da aba Arquivos (vira `@caminho`). Arquivo arrastado do Explorer do Windows e Ctrl+V de um print de verdade não conferidos |
| 1.6 Comandos de barra | `/nova`, `/parar`, `/diff`, `/aplicar`, `/descartar`, `/sessoes`, `/modelo`, `/ajuda` e os comandos da CLI do claude | ✓ na janela: menu com os do Argus e os da CLI, `/ajuda`, `/diff`; `/modelo` abre a lista de modelos da pílula (visto); `/nova` abre uma conversa nova e `/diff` abre a aba Alterações do painel direito (não reconferidos depois do redesenho). Desde 2026-11-18, a barra abre o menu **em qualquer ponto da mensagem** (começo, depois de um espaço ou de um parêntese), como o `@`, e a escolha troca só o trecho sob o cursor; os comandos do Argus continuam executando apenas quando a barra é a primeira coisa escrita (no meio da frase, o texto vai inteiro ao agente). Não conferido na janela |
| 1.7 Chamada de ferramenta escrita como texto | aviso com o botão "Executar de verdade" | ⚠ compila; detecção testada por script |
| 1.8 Uso do dia | pílula "hoje: X tokens · ≈ US$ Y (equivalente em API)" | ✓ na janela (status bar; atualiza a cada turno) |
| 2.4 (atalho) Detecção de loop | cancela o turno quando o agente repete a mesma chamada; só vale com a conversa aberta numa aba | ✓ na janela com agente real: o turno foi cancelado no terceiro `Read` igual, com o aviso "Parei: o agente repetiu Read 3 vezes seguidas" |

Ficou pendente do Digitama:

- **1.9 Conexões e cartão do repositório:** não feito (é o passo 9 abaixo).
- **Imagens anexadas com o claude:** resolvido. Os anexos ficam em `%APPDATA%\argus\anexos\<tarefa>\` (ou `novo\`, antes de a tarefa existir), fora da pasta de trabalho do agente; o core passa `--add-dir` com a pasta da tarefa e com as pastas dos anexos citados no pedido (`crates/core/src/attachments.rs`). Conferido numa tarefa real: o claude leu a imagem colada. Conversas novas usam `novo\<rascunho>\`, uma subpasta por aba, e o `--add-dir` libera só ela (conferido no código em 2026-11-16).
- **Comandos de barra da CLI:** na primeira vez que você digita `/` numa pasta, o Argus sobe um `claude -p --no-session-persistence`, manda `/context`, lê a linha de inicialização e mata o processo. Conferido contra a CLI real (claude 2.1.220), deixando o comando terminar: a resposta vem da própria CLI, com 0 tokens e custo 0. Para desligar mesmo assim, tire `chat_agent_capabilities` do `invoke_handler` em `src-tauri/src/main.rs`.
- **Comando de barra que o Argus não conhece** (por exemplo `/context`) continua indo para a CLI como mensagem, e a resposta aparece na conversa. Os que a CLI resolve sozinha (nenhum modelo chamado, tudo zerado) não geram linha de uso: não entram nas métricas nem na contagem de turnos da tarefa. Conferido com `/context` numa tarefa real.
- **"Turnos" nas métricas:** resolvido. Cada linha de uso leva o turno que a gerou (coluna `turn`), e as métricas contam turnos distintos.
- **`/modelo`:** resolvido. Abre a pílula de modelo; `/modelo nome` troca direto se a CLI informar esse nome.
- **`@agente`** vai como texto; os agentes embutidos de cada CLI não entram na lista.
- **Fila e anexos** vivem só na memória da janela: não sobrevivem a reinício nem aparecem no remoto.
- **Realce de sintaxe:** feito (2026-11-16, `src/views/chat/highlight.ts`, sem dependência nova; cores das variáveis existentes). A função foi conferida dentro da janela; um bloco de código numa conversa real ainda não foi visto.
- **Detecção de loop no core:** feita (2026-11-16, `crates/core/src/agents/loop_guard.rs`): 3 chamadas iguais seguidas (nome + entrada) encerram o turno com "Parei: o agente repetiu X 3 vezes seguidas", com a aba fechada e no remoto. Só testes unitários. **Pendente:** o `RepeatDetector` do frontend (`src/views/chat/toolcall.ts`, usado em `src/views/session`) continua ligado e pode dar aviso duplicado; tirar a parte de "seguidas" dele.
- **Ctrl+C no Explorer + Ctrl+V** anexando pelo caminho real.
- **Licença:** a lógica de menções e fila veio da versão reescrita do Digitama, que cita um projeto MIT de terceiros. Conferir antes de distribuir.

## Pendências de validação

Marcado com `[x]` o que foi visto funcionando na janela em 2026-11-14.

- [x] Primeiro envio numa conversa nova: criar a tarefa e a mesma aba virar a sessão (com stream ao vivo, botão Parar e fila).
- [x] "N agentes rodando" na status bar e o ponto pulsando na barra esquerda com um agente de verdade; negrito de "não lida" com a aba fora de foco.
- [x] Pílula de auto-work ligando e desligando o projeto; seta para cima recuperando o pedido anterior.
- [x] Arrastar um arquivo da aba Arquivos para a caixa de composição (vira `@caminho`).
- [x] Terminal com as cores do tema.
- [x] Arrastar abas entre grupos e para a borda; colar imagem na caixa (evento sintético).
- [ ] Arrastar um arquivo do Explorer do Windows para a conversa e Ctrl+V de um print de verdade (dependem do sistema; o protocolo de depuração não cobre).
- [x] Aplicar e descartar uma tarefa com alterações (claude).
- [ ] Aplicar e descartar uma sessão do OpenCode (ver o defeito do worktree preso, abaixo).
- [x] Auto-work rodando um ciclo na interface nova.
- [ ] Auto-work: Pausar, Retomar, Parar, travas e verificação de build/testes na janela.
- [ ] Player tocando música (só a detecção da mídia do sistema foi vista).
- [x] Login pela CLI num terminal (OpenCode; aberto e fechado sem concluir).
- [ ] Login do claude num terminal (abre o navegador; não foi aberto).
- [x] Tarefa pelo OpenCode.
- [x] Web app aberta num celular de verdade (2026-11-19, SM-M556B, Android 16, APK debug 0.1.2): pareamento pelo Funnel (`https://<pc>.ts.net/pair?token=…`), lista de sessões, histórico de uma sessão, aba Servidores e indicador "ao vivo". Ao desligar, `tailscale funnel status` ficou sem configuração. Corrigidos no teste: biblioteca nativa sem alinhamento de 16 KB (aviso do Android 16; `src-mobile/build.rs`), "Último computador usado" vazio aparecendo sem nada salvo (`[hidden]` vencido pelo `section { display:flex }`), texto do rodapé que dizia que nada ia para a internet, números em formato inglês na web app.
- [x] App do celular: a linha `FORTIFY: pthread_mutex_lock called on a destroyed mutex` no buffer de crash só apareceu num `am force-stop`; `src-mobile/src/lib.rs` só monta o Tauri (sem thread nem mutex do Argus), então é do encerramento forçado do runtime, não do app. Fechado como externo.
- [x] Servidor vinculado a uma sessão de agente.

## Defeitos conhecidos

Corrigidos (`[x]` = visto funcionando; `[ ]` = corrigido no código, falta ver):

- [x] "Enviar" em Nova tarefa não abria a aba da sessão. A conversa nova cria a tarefa e a mesma aba vira a sessão (visto com tarefas reais).
- [x] As métricas contavam 2 turnos quando um turno usava dois modelos (teste automático; na janela, 8 turnos para 11 linhas de uso).
- [x] O botão "Fazer login" aparecia mesmo com o provedor já conectado. Agora, conectado, aparece só "Trocar de conta" (visto na aba Provedores).
- [x] Imagens anexadas não eram lidas pelo claude, por ficarem fora da pasta do projeto: `--add-dir` com as pastas de anexos (visto numa tarefa real).
- [x] O aviso da CLI do claude sobre servidores MCP bloqueados mostrava o nome da organização do usuário no chat. As linhas que começam com "Warning: claude.ai MCP servers blocked" não são mais renderizadas (`isHiddenNotice` em `views/session/events.ts`). O texto continua gravado no banco; no acesso remoto ele é filtrado no servidor e não chega ao celular.
- [x] Um comando de barra resolvido pela própria CLI contava como turno e entrava nas métricas (visto com `/context`).
- [ ] "Hoje" nas métricas virava às 21h de Brasília. Corrigido no código (os dias seguem o fuso do computador) e coberto por teste automático com fuso fixo; **a virada do dia ainda não foi vista na janela** (confira depois das 21h).
- [x] Uso contado em dobro: quando um comando em segundo plano termina depois da resposta, o `claude -p` acorda e imprime um segundo `result` com o total ACUMULADO do processo. O parser agora desconta o que o `result` anterior já informou (teste com os números da execução real).
- [x] O OpenCode trabalhava na pasta errada quando o Argus era aberto por um shell (Git Bash, `npm run tauri dev`): a CLI confia na variável `PWD`, herdada de quem abriu o Argus, em vez do diretório real. O Argus agora apaga `PWD` ao iniciar e a acerta em cada processo filho (visto: antes "README.md não encontrado", depois a resposta certa).
- [x] "Trocar de conta" do OpenCode falhava com "não é um aplicativo Win32 válido (erro 193)": o terminal executava o script sem extensão do npm. O terminal agora resolve o nome (`.exe` antes de `.cmd`) e roda `.cmd`/`.bat` por `cmd /c` (visto).
- [x] A aba Alterações mostrava "O nome do diretório é inválido (erro 267)" depois de aplicar ou descartar. Sessão encerrada não calcula mais o diff (visto).
- [x] "hoje: … US$ -0,00" na status bar sem uso no dia (visto).
- [x] A aba ativa ficava meio escondida debaixo dos botões da direita quando a faixa de abas transbordava (visto).
- [ ] O selo de auto-work oferecia "Reiniciar auto-work" numa sessão já aplicada ou descartada. Corrigido no código, **ainda não revisto na janela** (depois de reiniciar o app o selo some, e o teste foi feito assim).
- [x] Não havia como abrir um terminal como aba do editor (o tipo existia, nenhum botão o criava): botão "⧉" na barra de terminais (visto, com a aba mudando de grupo).

- [x] O agente respondia "não achei a pasta" depois de vários passos: o worktree só tem o que está commitado e nada avisava. Agora há o aviso antes do envio, o modo "Direto na pasta" e o agente isolado sabe onde está (visto com tarefas reais: de 8 passos e US$ 1,82 para 2 passos e US$ 0,15 com o modelo haiku).
- [x] O agente não conseguia rodar `git` nem comando nenhum: no Windows o claude usa a ferramenta `PowerShell`, e só `Bash(...)` da lista do projeto era liberado. Conjunto padrão para as duas ferramentas (visto com tarefa real).
- [x] A lista de escolha (`Select`) com busca não recebia o foco ao abrir: digitar para filtrar e as setas não funcionavam (o foco era pedido antes de a lista existir). Corrigido e visto.
- [x] A lista de escolha fechava quando a janela perdia o foco, inclusive no meio da consulta de modelos à CLI. Não fecha mais por perda de foco.

Em aberto:

- [ ] (2026-11-16) Formatos misturados. **Corrigido fora da conversa:** `formatIntPt`, `formatTokensPt` e `formatUsdPt` em `src/api/util.ts`, usados nas Métricas (vistas na janela em pt-BR), mapa de agentes, auto-work, pet e chat. **Falta em `src/views/session`:** `index.tsx` (tooltip do total com números crus e `formatTokens`/`formatUsd`), `Transcript.tsx` e `ContextMeter.tsx`; trocar pelas versões `Pt`.
- [ ] (2026-11-16) Modelo gratuito do OpenCode aparecia com custo "equivalente em API". Corrigido: nomes terminados em `-free`, `:free` ou `/free` custam 0 (`metrics.rs`, com teste). Falta ver com o opencode real.
- [ ] (2026-11-16) Web app do celular passou a usar Geist, embutida no binário e servida em `/geist.woff2` (atrás da mesma autenticação). A escala tipográfica não mudou (não há escala nos tokens além de `--font-ui`). Não vista num aparelho.
- [ ] (2026-11-16) A primeira tarefa haiku de um projeto gravou 73 mil tokens de cache (US$ 0,16 equivalentes para listar 3 arquivos); as seguintes leram o cache. Custo da CLI, não do Argus, mas vale avisar na pílula de modelo.
- [ ] Trocar o modelo de uma sessão do claude regrava o cache da conversa inteira no modelo novo: o primeiro turno depois da troca sai caro (visto: US$ 0,64 equivalentes em API). **2026-11-16:** a lista de modelos de uma sessão do claude com histórico mostra o aviso; falta ver na janela.
- [ ] `cd <pasta>; git status` escrito pelo agente no PowerShell foi negado numa tarefa real (o `cd` não estava liberado). `cd` e `Set-Location` entraram no padrão, mas a correção não foi vista funcionando: no teste seguinte o agente se recusou sozinho a encadear o `cd`.
- [ ] OpenCode: permissões de comando seguem a configuração de agentes do usuário (com regras "ask" e "deny" próprias); o Argus não as altera. Num `opencode run` sem interação, o que cai em "ask" não tem quem aprove. Não conferido com um comando git real (o turno de teste terminou sem chamar ferramenta).
- [ ] O aviso de ambiente do modo Isolado não impede o agente de procurar: no teste ele ainda fez 2 buscas antes de responder.
- [ ] Web app do celular: a divisória de troca de agente, a linha "Comando negado" e o provedor/modelo no subtítulo foram escritos e passam no `node --check`, mas não foram vistos numa tela.
- [ ] Turno cancelado não registra uso. **2026-11-16:** corrigido no código: soma o `usage` das mensagens `assistant` vistas depois do último `result` e grava uma linha parcial (custo estimado pela tabela). Teste automático; falta ver com um turno real cancelado.
- [ ] OpenCode 2.0.22 não imprime o `step_finish` do último passo no `run --format json` (numa tarefa real: 10 959 de 11 547 tokens de entrada registrados). **2026-11-16:** o resultado agora é sintetizado no fim do processo; o uso do último passo só entra se outra linha trouxer os tokens (nenhum número inventado), então a diferença pode continuar. Falta ver com o opencode real.
- [ ] `which_bin` agora segue a ordem do PATH e, em cada pasta, a da PATHEXT (antes: `.exe` sempre antes de `.cmd`). Coberto por testes com pastas falsas; não conferido com os dois opencode reais instalados.
- [ ] Worktree preso por processo da sessão: Aplicar e Descartar agora encerram os processos ligados ao worktree (árvore da sessão, cwd ou linha de comando dentro da pasta), tentam a remoção 4 vezes com espera curta e, por fim, `--force --force` + `prune`; se a pasta continuar, dão erro e mantêm a branch e o estado (dá para repetir). Seleção dos processos coberta por teste; não conferido com o serviço real do opencode.
- [ ] O servidor aberto pelo claude numa sessão morre quando o turno termina (a CLI encerra os processos em segundo plano). Para um servidor que sobreviva ao turno, o agente precisa iniciá-lo desvinculado, o que pede outro comando permitido.
- [x] `anexos\novo\` era compartilhada por todas as conversas novas: o `--add-dir` dessa pasta deixava o agente de uma conversa ler os anexos recentes das outras. **2026-11-16:** cada aba de conversa nova tem a sua subpasta `anexos\novo\<id>\` (visto na janela com duas abas).
- [ ] Edição de arquivos pelo OpenCode v2 não conferida: sem `--auto`, o `run` pode negar a permissão de escrita.
- [ ] Um turno retomado gravou um evento de resultado vazio (ok, custo 0) logo no começo. **2026-11-16:** `result` de sucesso sem texto, sem uso novo e custo 0 não vira mais evento. Causa deduzida (não reproduzida): `result` de zero turnos do processo retomado ou repetido com o mesmo acumulado.
- [ ] Pareamento pelo celular dava "Não autorizado" ao abrir o link do QR (visto pelo Tailscale). Causa provável: cookie `SameSite=Strict`, que o navegador do celular não envia quando o link vem da câmera ou de outro app. Corrigido no código (cookie `Lax` e página de pareamento que segue sozinha para a tela inicial); **falta confirmar num aparelho**.

## Codex e Gemini (2026-11-16)

**Estado: compila e os testes do parser passam; não exercitado na janela** (nenhuma das duas CLIs estava instalada na máquina de desenvolvimento).

| | Codex (`@openai/codex`, `codex`) | Gemini (`@google/gemini-cli`, `gemini`) |
|---|---|---|
| Código | `crates/core/src/agents/codex.rs` | `crates/core/src/agents/gemini.rs` |
| Login | `codex login` (conta ChatGPT); status por `codex login status` | `gemini` interativo ("Login with Google" ou `/auth`); sem comando de status: lê só o tipo escolhido em `~/.gemini/settings.json` e a existência de `oauth_creds.json`/`GEMINI_API_KEY`, nunca o conteúdo |
| Turno | `codex exec --json --sandbox workspace-write -c approval_policy=never --skip-git-repo-check -C <pasta> [-m modelo] [resume <thread_id>] -` | `gemini --output-format stream-json --approval-mode auto_edit --skip-trust [-m modelo] [--resume <session_id>]` |
| Pedido | entrada padrão (`-`) | entrada padrão (modo sem interface porque a entrada não é terminal) |
| Sem aprovação | edição e comandos de shell DENTRO do sandbox (escrita só na pasta de trabalho, sem rede); o que o sandbox barra falha (vira `Denied`) | só edição de arquivos na pasta de trabalho; shell e o resto que pediria aprovação é negado pela CLI |
| Modelos | lista fixa (gpt-6.1-sol, gpt-6.1-terra, gpt-6-astra, gpt-6-luna) + digitação | `gemini --list-models --output-format json`; se falhar, lista fixa |
| Contexto | só em turno sem ferramentas (o `usage` é soma dos pedidos); janela conhecida só do gpt-6.1-sol (500 mil) | idem; janela de 1 milhão |
| Custo | tabela oficial da OpenAI; o codex não informa o modelo, então "Padrão da CLI" cai no fallback | tabela oficial do AI Studio, uma linha de uso por modelo |

Isolado: os dois recebem o aviso da cópia na frente do primeiro pedido (como o opencode). Anexos do chat ficam fora da pasta de trabalho e não são liberados para eles (`--add-dir` é só do claude).

Falta:
- instalar (botão "Instalar..." em Provedores, que usa o fluxo de Dependências), fazer login e rodar uma conversa de cada na janela: conferir o stream real, retomada, troca de agente e custo;
- conferir as suposições: total acumulado do `turn.completed` do codex numa retomada (o Argus desconta o último total visto da thread; depois de reiniciar o app, o primeiro turno retomado pode contar o total inteiro), raciocínio dentro de `output_tokens`, `--skip-trust` aceito pela versão instalada do gemini, `gemini` sem `-p` entrando no modo sem interface;
- sandbox do codex no Windows (é experimental na CLI): verificar se `workspace-write` realmente bloqueia escrita fora da pasta;
- "Liberar este comando neste projeto" não vale para o codex (a lista de comandos permitidos é do claude).

## Tarefas agendadas e "Iniciar com o Windows" (2026-11-19)

Branch `feat/tarefas-agendadas`. **Estado: core testado; compila; a tela não foi exercitada na janela.**

- **Core** (`crates/core/src/schedules.rs`, tabela `schedules(id, data JSON)` em `db.rs`): nome, tipo (pedir a um agente ou só lembrar), projeto, provedor, modelo, modo de trabalho (ausente = padrão do projeto), pedido, recorrência (`Once` em data/hora, `Daily` HH:MM, `Weekly` dias + HH:MM, `EveryHours` contadas da criação, `Cron` de 5 campos com `*`, listas, faixas e `/passo`), ligado, última e próxima execução, id da última tarefa, último resultado e "atrasada". Horários no fuso do computador (`chrono::Local`); hora inexistente na troca de horário de verão vira a primeira hora que existe depois.
- **Laço** (`Scheduler::start`, a cada 30 s, a primeira volta logo ao abrir): o que venceu dispara criando uma tarefa normal por `Orchestrator::submit_with` (mesmo caminho de "Nova conversa": histórico, uso e permissões iguais). Venceu com o Argus fechado: roda UMA vez, marcada "atrasada", e a próxima conta a partir de agora (nada se acumula). "Uma vez" desliga depois de rodar. Lembrete puro só mostra a notificação.
- **Notificação do sistema** (`src-tauri/src/commands/schedules.rs`, toast do Windows pelo crate `windows`, sem dependência nova): lembrete, tarefa agendada concluída, falhou, cancelada ou que nem iniciou. O app instalado usa o AUMID `dev.argus.app`; a cópia de desenvolvimento usa o do PowerShell (como o plugin oficial faz). O fim de turno das conversas comuns continua sem notificação do sistema.
- **Tela**: área "Tarefas" no rail (ícone de calendário com relógio): lista (nome, projeto · agente, recorrência em pt-BR, próxima, último resultado com "Abrir conversa"), ligar/desligar, "Executar agora", apagar (confirmação nativa) e o painel de criar/editar ao lado. Paleta: "Nova tarefa agendada". Evento `schedules-changed` atualiza a lista.
- **Iniciar com o Windows**: Configurações › Este computador › Inicialização. Grava `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` valor `Argus` = `"<exe>" --minimizado` pelo `reg.exe` (sem plugin, sem elevação); com `--minimizado` a janela abre minimizada. No perfil dev (`argus-dev`, qualquer build de depuração) a opção fica desabilitada com o motivo e o comando recusa.
- **Falta ver na janela**: criar/editar cada tipo de recorrência, "Executar agora" criando a conversa, o link "Abrir conversa", o toast aparecendo (dev e instalado), recuperação ao abrir depois de um horário perdido, e o registro de verdade no app instalado (reiniciar o Windows e ver abrir minimizado).

## Funções da extensão do VS Code portadas (2026-11-19, branch `feat/extensao-vscode`)

Valem para qualquer provedor. Estado: **core testado** (Rust) e **compila** (tela; nada visto na janela).

- **Acesso por conversa: Planejar / Normal / Bypass** (`permissions::AccessMode`, coluna `access` da tarefa, `Orchestrator::set_access`, comando `task_set_access`). Escolha em "Opções da conversa" do composer, depois do primeiro envio. Argumentos por provedor (cada `task_command` lê `TurnOptions::access`):
  - Planejar: claude `--permission-mode plan` (se o `--help` tiver `"plan"`; senão fica `acceptEdits`), gemini `--approval-mode plan`, codex `--sandbox read-only`, opencode `--agent plan`. Ao fim do turno, "Executar o plano" volta ao Normal e manda "Execute o plano que você propôs acima.".
  - Bypass: claude `--permission-mode bypassPermissions` (ou `--dangerously-skip-permissions` se a CLI não listar `bypassPermissions`), codex `--dangerously-bypass-approvals-and-sandbox` (sem `--sandbox`), gemini `--approval-mode yolo`, opencode `OPENCODE_CONFIG_CONTENT={"permission":"allow"}`. A política de shell do Argus (`permissions::apply`) não entra (`apply_for`). Nunca padrão, só na conversa onde foi ligado, confirmação pelo diálogo do sistema, selo vermelho "Bypass" no cabeçalho, na aba e na barra de status.
  - ⚠ conferido só a montagem: o claude 2.1.x lista `plan`/`bypassPermissions` no `--help`; codex e gemini não estão instalados aqui (flags da documentação); `opencode run --agent plan` e a config `permission: allow` não rodados.
- **Editor no chat**: Alt+K (e menu de contexto do Monaco) "Adicionar à conversa" insere `@caminho#L10-20` e o trecho na conversa visível. Acima do composer, chip "arquivo em foco · L10-20" (ligado por padrão; clique desliga) anexa o arquivo/seleção ao pedido, e chip "N erros do editor" (só com markers do Monaco; desligado por padrão) anexa os erros/avisos. Estado em `src/views/workbench/editorContext.ts`.
- **Rejeitar no diff**: `Orchestrator::revert_file` (volta ao último commit com `git restore --worktree`, ou apaga o arquivo novo) e `revert_hunk` (patch do trecho tirado do `git diff HEAD`, aplicado com `git apply -R` só nos arquivos), no worktree ou na pasta (Direto). Na aba de alterações: "Rejeitar arquivo" e "Rejeitar trecho acima" (quando há mais de um trecho), com confirmação. Bloqueado com turno rodando.
- **"Liberar e repetir"** no cartão Comando negado: libera o comando exato e manda o turno curto "o comando X foi liberado; rode de novo".

Falta ver na janela: menu de opções com os três modos e o diálogo do Bypass; selos na aba e na barra de status; "Executar o plano" depois de um turno real em Planejar; Alt+K com seleção e o chip de foco atualizando ao trocar de arquivo/grupo; chip de erros num .ts com erro; rejeitar trecho num arquivo CRLF; "Liberar e repetir" numa negação real.

Feito em 2026-11-19: escolher Planejar/Bypass antes do primeiro envio (`SubmitOptions::access`, `task_create` com `access`). Codex e gemini leem a memória: `--add-dir` / `--include-directories` só com a pasta `_Argus` (flags da documentação, CLIs não instaladas aqui). O ponto de "rodando" não pulsa mais (anel fixo).

Ficou de fora: checkpoints por turno ("Voltar a este ponto", ref `refs/argus/t<id>/<n>` com `git stash create` antes do turno); Alt+K quando não há conversa aberta (o texto só entra se uma aba de conversa estiver visível).

## Próximos passos (em ordem)

1. **Terminar o teste na janela.** O que ainda não foi conferido:
   - arrastar um arquivo do Explorer do Windows e Ctrl+V de um print de verdade (com o mouse e o teclado reais);
   - aplicar e descartar uma sessão do OpenCode; edição de arquivos pelo OpenCode;
   - login do claude num terminal;
   - player tocando mídia e os controles dele;
   - auto-work: Pausar, Retomar, Parar, travas e verificação de build/testes.
2. **Fechar os defeitos em aberto** (lista acima), a começar pelo uso de turno cancelado, pelo worktree preso do OpenCode e pelo uso do último passo do OpenCode.
3. **Abrir a web app num celular de verdade.** Na primeira vez, o Windows pede permissão no Firewall: clique em **Permitir** (rede privada). A web app foi refeita e conferida num navegador emulando celular; falta o aparelho real (teclado de verdade, área segura, "Adicionar à tela inicial", toque). Conferir também o auto-work com uma execução real e o primeiro envio de Nova conversa.
4. **Subagentes: conferir o que foi suposto.** Uma saída real do opencode com a ferramenta `task`, e um subagente em segundo plano do claude.
5. **Pequenos ajustes conhecidos:**
   - O botão "Reiniciar" de um servidor roda sem saída visível. Seria melhor reabrir o processo num terminal da IDE.
   - Janela separada de servidores não foi portada (a permissão para janelas `aux-*` já existe).
   - Trocar o `netstat` por GetExtendedTcpTable, o que exige adicionar a crate `windows` ao core.
   - Espaçar as verificações de build e testes do auto-work em projetos grandes, ou torná-las opcionais.
   - O OpenCode não informa o modelo usado: a linha de uso leva o modelo escolhido na pílula; com "Padrão da CLI" o custo cai na tabela de fallback.
   - Sessão sem `session_id` (primeiro turno falhou antes de a CLI abrir a sessão) continua sem resposta possível; daria para reaproveitar o resumo da troca de agente para recomeçar.
   - O modo de trabalho de uma sessão existente é fixo; "continuar esta conversa direto na pasta" pediria copiar o worktree ou abrir outra tarefa com o resumo.
   - A tabela de preços não tem o modelo "fable": vale o custo informado pela CLI; sem ele, cairia no fallback.
   - Web app do celular: HTTPS (para instalar como PWA de verdade), miniaturas dos anexos e fila de mensagens.
   - A fila de mensagens e o rascunho da "conversa nova" são compartilhados entre todas as abas de conversa nova (chave `new`).
   - `OrchestratorEvent` não deriva `Serialize`; por isso existe o espelho `OrchMsg` em `src-tauri/src/events.rs`. Derivar no core elimina o espelho.
   - TypeScript/JavaScript no Monaco só valida sintaxe (a validação semântica foi desligada).
6. **Empacotamento:** `npm run tauri build` gera o instalador. O Tauri avisa que o identificador `dev.argus.app` termina em `.app`, o que conflita com bundles do macOS; no Windows não afeta.
7. ~~Apagar `crates/app`~~: feito (commit `14d5fa6`).
8. **Uso de CPU e GPU por sessão.**
   - Feito no core e no painel Servidores (2026-11-16): `crates/core/src/usage.rs` (sysinfo + WMI `GPUPerformanceCounters_GPUEngine`, sem janela), comando `owners_usage`, componente `src/views/servers/SessionUsage.tsx`. Comando conferido na janela (lista vazia sem sessão rodando); números com uma sessão real não vistos. **Pendente:** pôr `<SessionUsage>` no cabeçalho da sessão (`src/views/session`); o painel só mostra donos que têm servidor.
   - Mostrar, em cada sessão, o consumo de CPU, memória e GPU dos processos dela (agente, subprocessos e servidores vinculados).
   - A árvore de processos por sessão já existe em `crates/core/src/procs.rs`; a CPU e a memória vêm do `sysinfo`.
   - GPU por processo no Windows: contadores de desempenho "GPU Engine" (PDH) ou `nvidia-smi` quando houver placa NVIDIA.
9. **Conexões com serviços de desenvolvimento.**
   - Feito (2026-11-16): `crates/core/src/connections.rs` (gh, gcloud, firebase, vercel: instalada e logada perguntando à própria CLI; linhas com "token" descartadas), comandos `connections_status` e `connections_repo`, tela `src/views/connections/` com "Fazer login" no terminal embutido e `RepoCard.tsx`. Botão "Conexões" no rodapé da barra esquerda; tela vista na janela (GitHub conectado, gcloud e firebase não instalados, Vercel com "Fazer login") e `connections_repo` conferido com o próprio repositório. **Pendente:** clicar em "Fazer login" (não foi clicado) e pôr o `RepoCard` no chat (`src/views/session`).
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
12. **Monitor de builds e instaladores.**
    - Feito (grupo C, 2026-11-16): `crates/core/src/runs.rs` (detecção, portas, artefatos), `src-tauri/src/commands/runs.rs` (execuções, watcher, histórico em `build_runs`), tela Builds no rodapé. Builds de fora dos botões são reconhecidos pela linha de comando (sem código de saída).
    - Acompanhar, por projeto, a geração de builds e de instaladores (por exemplo `cargo build`, `npm run build`, `npm run tauri build`, `gradle assemble`, `flutter build`): em andamento, concluído ou falhou, com duração, saída e o caminho e tamanho do artefato gerado.
    - Detectar os comandos de build de cada projeto pelos arquivos dele (scripts do `package.json`, `Cargo.toml`, `tauri.conf.json`, Gradle e outros) e oferecer um botão para cada um.
    - Histórico dos builds por projeto e aviso (pet, status bar e celular) quando um build termina ou falha.
    - Vale para builds iniciados pelo usuário num terminal da IDE e pelos agentes.
13. **Servidores de dev com um clique.**
    - Feito (grupo C, 2026-11-16): seção "Servidores de dev" no painel Servidores, botão no hover do projeto e no menu dele; porta livre mais próxima quando o comando aceita porta; último comando em `settings` (`devserver:<projeto>`).
    - Detectar os comandos de servidor de desenvolvimento de cada projeto (`npm run dev`, `npm run tauri dev`, `cargo run`, `vite` e outros) e mostrar um botão Iniciar/Parar por projeto, na barra esquerda e no painel Servidores.
    - Saber quando dá para abrir: bloquear ou avisar enquanto um build do mesmo projeto estiver rodando ou a porta estiver ocupada; mostrar "pronto" quando a porta começar a responder, com o botão Abrir no navegador.
    - Se a porta estiver em uso, escolher a livre mais próxima quando o comando aceitar a porta por parâmetro.
    - Lembrar o último comando usado em cada projeto e permitir "iniciar todos" de um conjunto.
    - Aproveita o rastreamento de processos e portas de `crates/core/src/procs.rs`.
14. **Memória como vault do Obsidian.**
    - Tratar a memória do Argus (notas, contexto e decisões que os agentes leem e gravam) como um vault do Obsidian: arquivos Markdown simples numa pasta, com links `[[nota]]` e frontmatter, abríveis e editáveis direto no Obsidian.
    - Hoje o vault é a pasta que o Argus controla, acima de todos os projetos: `D:\Dev\Desenvolvimento` já tem `.obsidian/`, e os `.md` com o nome de cada projeto em `Projetos\` (ex.: `Argus.md`, `Digitama.md`) são notas do vault. O Argus ainda não lê nem grava nada nele.
    - Feito (2026-11-19): preferências globais e memória em `<raiz>/_Argus/` (não `Argus`: na raiz real essa é a pasta do repositório do Argus; `workspace::scan` não lista `_Argus` como projeto) (a raiz é a pasta aberta no Argus, nada fixo no código). `Preferencias.md` é o prompt que vale para todos os projetos e provedores; `Memoria.md` é o índice; `Memoria/*.md` são as notas, uma por fato, com frontmatter `tipo`, `projetos`, `atualizado` e links `[[nota]]`. Editável em Configurações › Preferências e memória (salvar cria a estrutura; lista das notas com busca, Abrir no editor e Abrir no Obsidian por `obsidian://open?path=`). O conteúdo não vai no prompt: o aviso de sistema (`crates/core/src/memory.rs`, `compose_note`) só aponta os arquivos, é estável entre turnos, sem aspas nem quebras, e a raiz inteira entra com `--add-dir` no claude (decisão do usuário, 2026-11-19: dá acesso à nota do projeto e às outras notas do vault). Se existir `<raiz>/<Projeto>.md`, o aviso aponta para ela também. O aviso só entra quando a pasta `_Argus` existe. A memória fica fora do git dos projetos.
    - Falta ver na tela e numa conversa real: a página nova; se o agente lê as preferências e grava notas; se o claude consegue ler `<raiz>/<Projeto>.md` (fora da pasta de trabalho e fora do `--add-dir`); se codex, gemini e opencode leem a pasta `Argus` fora do diretório de trabalho; o "Abrir no Obsidian" com o Obsidian instalado.
15. **Eficiência de tokens (2026-11-19).**
    - Feito: o claude roda com `--exclude-dynamic-system-prompt-sections` (o git status saía do prompt de sistema e regravava o cache da conversa inteira a cada turno; a sessão #9 regravou ~10,9 M tokens em 21 turnos) e com `--include-partial-messages` (o texto aparece enquanto é gerado, como `TextDelta`, que não é gravado no banco nem enviado ao celular).
    - Feito (2026-11-19): compactação automática. Antes de um turno de resposta do claude, se o contexto do último pedido passou do limite (Configurações › Contexto; padrão 200 mil, 0 desliga), o Argus roda `/compact` com `--resume` e só então o turno. Conferido na CLI 2.1.220: 39 252 → 2 663 tokens e a sessão lembrou o contexto. O `compact_boundary` vira a divisória "Conversa compactada" no chat. **Falta ver numa sessão longa real dentro do app.**
    - Pendente: as opções novas do claude exigem uma CLI recente (conferidas na 2.1.220); com uma antiga, todo turno falharia. Checar a versão antes de passar as opções.
    - Pendente: "Padrão da CLI" hoje resolve para o modelo de 1M; avaliar um padrão de janela menor para conversas comuns.
    - Pendente: medir de novo uma sessão longa com as duas opções e comparar `cache_write` por turno.
16. **Refazer o ícone do app (não crítico).** O olho atual é provisório; redesenhar o ícone do Argus (janela, barra de tarefas, instalador e web app do celular).
17. **Pendência visual do critique: aba Celular com outra linguagem visual.** O painel do Screen-Mirror (iframe) não aceita tema: `web/app.js` não lê query string nem escuta `postMessage`. A faixa do Argus em volta já usa os tokens; o miolo continua com o visual do Screen-Mirror. Para alinhar sem tocar no CSS dele por fora, o Screen-Mirror precisaria expor:
    - parâmetros de tema na URL (ex.: `?accent=%23778da9&bg=%230d1b2a&surface=%231b263b&text=%23e0e1dd&radius=6`) aplicados como variáveis CSS na raiz;
    - ou um `window.addEventListener("message")` que aceite `{ type: "theme", tokens: {...} }` só da origem do Argus, para trocar o tema sem recarregar quando o usuário muda de tema;
    - o `web/style.css` dele já tem variáveis na raiz (`--bg`, `--surface`, `--border`, `--text`, `--muted`, `--accent`, `--accent-text`), mas claras e sem raio; bastaria sobrescrevê-las pelos dois caminhos acima, somar `--radius` e ter um modo escuro.

## Notas

- Os worktrees dos agentes ficam em `<projeto>/../.argus-worktrees/`. Worktrees antigos em `.harness-worktrees` podem ser apagados com `git worktree prune` no projeto de origem.
- A pasta do projeto foi renomeada de `Harness` para `Argus`. As sessões antigas do Claude Code ficaram registradas sob o nome antigo.
- O `index.html` não pode ter `<style>` inline: o `vite build` falha quando o diretório atual vem com a letra do drive minúscula (`d:\`).
- App instalado x desenvolvimento: o instalado (`npm run app:instalador`, NSIS em `target\release\bundle\nsis\`) usa `%APPDATA%\argus\`; `npm run app:dev` abre o "Argus Dev" com `%APPDATA%\argus-dev\`. O app instalado foi instalado e conferido pelo usuário em 2026-11-16.

## Decisões fixas

- Os agentes são as CLIs oficiais, com o login da assinatura. Nada de chave de API, e a IDE não lê tokens.
- O claude roda com `--permission-mode acceptEdits`. O shell (Bash e PowerShell) fica bloqueado, a não ser o conjunto padrão (git de leitura e inspeção), os níveis "Git local" e "Git remoto" quando ligados e o que estiver em "Comandos permitidos" por projeto. Nunca entra shell genérico nem `--add-dir` da pasta original do projeto.
- Nenhuma marca ou nome de empresa no projeto. O identificador do app é `dev.argus.app`.
- O custo é sempre exibido como "equivalente em API".
- Textos da interface em português do Brasil. Windows primeiro.

## Integração de outros projetos

Os planos para unificar Screen-Mirror, Gerenciador de Portas e AllSeenEye no Argus estão em [PENDENCIAS.md](PENDENCIAS.md).

## Critique 3 e revisão de motion (2026-11-19)

Pendentes do critique 3 (27/40; snapshot em `.impeccable/critique/`):
- [x] Popover de opções do composer em texto corrido; "Modo de permissão" (só leitura) compete com "Acesso nesta conversa" (`src/views/session/pills.tsx`).
- [x] Pendências: recolher os itens feitos numa linha "N feitos" e mostrar os abertos primeiro (`src/views/session/ChecklistPanel.tsx`).
- [x] Início: "Próximas agendadas" e "Pendências abertas" no lugar do vazio; legenda das bolinhas.
- [x] (feito: rótulos de estrutura a 11px; nomes de projeto viraram conteúdo, 12,5px sem caixa-alta) Texto de 10,5px (nomes de projeto, cabeçalhos de tabela, Ctrl K) e `.meta` de item selecionado com 3,7:1.
- [x] Faixa de abas: botões cobrem o título da última aba com o inspetor aberto. (só as abas rolam, em `.wb-tabs-list`; os botões ficam fora)
- [x] Diálogo do Bypass com ícone de aviso em vez de informação. (`system_confirm` ganhou `warn`)

Motion (revisão só pelo código; falta captura quadro a quadro):
- [x] Chaves por posição na linha do tempo ao vivo (`Transcript.tsx`, `r${i}`/`s${i}`): itens trocam de conteúdo no lugar e a entrada roda de novo; usar a posição global do item.
- [x] (o rascunho agora entra no `.tl` do último turno, via `liveItem`) Rascunho (TextDelta) vira texto final com fade/salto: reaproveitar o nó.
- [x] Ao trocar de conversa todo o histórico anima: só animar o que chega depois do primeiro paint.
- [~] (parcial: sem o brilho no texto "trabalhando"; pet em rajadas de 3) 6–7 laços de "rodando" ao mesmo tempo; deixar o halo como único laço focal. Pet em laço infinito.
- [x] Tokens de duração e curva em `tokens.css`; entrada curta na paleta, menus e painel de Nova tarefa.
