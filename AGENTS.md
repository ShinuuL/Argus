# Argus — regras para agentes

Este é o arquivo de regras do repositório. É a fonte única de verdade para
escopo, comandos de verificação, arquitetura e armadilhas. Leia antes de
escrever código. Um `CLAUDE.md`, quando existir, só aponta para cá (`@AGENTS.md`).

O Argus é uma IDE desktop que abre uma pasta raiz de projetos e orquestra
agentes de IA (Claude Code, OpenCode, Codex e Gemini). O core é Rust
(`argus-core`); a interface é React + TypeScript numa janela Tauri 2. Os
agentes usam o login das próprias CLIs oficiais: a IDE nunca lê nem guarda
tokens deles.

## Autorização e escopo

- Agentes não podem criar, editar ou excluir código, configuração,
  dependências ou documentação sem pedido direto do desenvolvedor para o
  escopo e a fase exatos. Inspeção e verificação somente leitura são livres.
- Não criar, editar ou excluir arquivos fora da raiz do repositório sem
  autorização explícita. Isso vale para worktrees, clones e pastas temporárias.
- Não adicionar dependência nova sem autorização explícita.
- Não fazer commit nem push sem pedido explícito.
- Não registrar segredo, token ou chave em código, log ou documentação. O token
  do acesso remoto aparece apenas como `?token=…` no link de pareamento.
- Documentação de estado e pendências fica em `PROXIMOS_PASSOS.md` e
  `PENDENCIAS.md`; itens ainda não feitos devem continuar marcados como `- [ ]`.

## Gates obrigatórios

Rode a partir da raiz do repositório, nesta ordem. Se um falhar, corrija e rode
de novo antes de declarar a tarefa concluída.

```powershell
npm run typecheck                 # tipos da interface (tsc --noEmit)
cargo test --workspace            # testes do core e do desktop
cargo check -p argus-desktop      # crate Tauri, quando a mudança toca a janela
npm run teste:celular             # web app do celular num DOM mínimo
npm run build                     # tsc + vite build, quando a mudança toca a interface
node --check crates/core/assets/remote/app.js   # sintaxe da web app do celular
```

`src-mobile` fica **fora** do workspace (alvo Android, SDK/NDK próprios). Não
tente verificá-lo com `cargo test --workspace`; use `npm run celular:apk`
dentro da pasta quando a tarefa pedir o APK.

## Como rodar

Requisitos: Rust stable (MSVC), Node.js 20+ e WebView2 (já vem no Windows 11).

```powershell
npm install
npm run dev                       # só o Vite
npm run app:dev                   # IDE com recarga automática da interface
npm run tauri build -- --debug --no-bundle
```

Variáveis úteis: `ARGUS_ROOT` (abre uma pasta raiz), `ARGUS_DB` (outro SQLite),
`ARGUS_ALLOWED_TOOLS` (outro arquivo de comandos permitidos),
`ARGUS_OPENCODE_MODEL` (modelo inicial das conversas novas do OpenCode).

Para validar na interface sem mouse: `ARGUS_PONTE=1 npm run app:dev` e
`node scripts/ui.mjs <ação>`. A ponte escuta só em `127.0.0.1:8787`, existe só
com a variável definida e não toca em arquivo nem no banco.

## Estrutura

```
package.json, vite.config.ts, index.html     front-end (Vite)
src/
  api/        uma função tipada por comando Tauri, eventos e tipos que espelham o serde
  store/      zustand: useApp, useLayout, useWorkbench, useTerminals, useTheme, bootstrap
  shell/      Shell, TopBar, RightPanel, BottomPanel, StatusBar, ContextMenu, icons
  styles/     tokens.css (cores por tema), components.css, shell.css
  views/      projects, session, chat, workbench, explorer, servers, agents,
              settings, autowork, deps, metrics, remote, terminal, player, pet
  dev/        galeria de estados e ponte de validação (só desenvolvimento)
src-tauri/    crate argus-desktop (Tauri 2, identificador dev.argus.app)
  src/commands/  workspace, monitor, fs, pty, agents, autowork, servers, deps,
                 metrics, remote, media, system, extra_agents, extra_chat, schedules
  src/events.rs  repassa eventos do orquestrador e dos servidores
src-mobile/   crate argus-celular (Tauri 2 Android, dev.argus.celular), fora do workspace
crates/core       (argus-core): workspace, monitor, pty, procs, deps, metrics, db,
                  remote, autowork, schedules, isolation, permissions, mural,
                  agents/{provider, claude, opencode, codex, gemini, orchestrator, handoff}
  assets/remote   web app do celular, embutida no executável por include_str
```

Monaco e xterm entram por npm e são empacotados pelo Vite. Nada vem de CDN.

## Arquitetura e regras de negócio

- **Comunicação:** a interface fala com o Rust só por `src/api/commands.ts`.
  Eventos passam por `src/api/events.ts`. Não chame `__TAURI_INTERNALS__`
  diretamente: no Tauri 2 as propriedades são não graváveis e não
  configuráveis, e um `defineProperty` nelas derruba o boot.
- **Temas:** Oceano, Nebulosa e Brasa. Toda cor vem de `src/styles/tokens.css`;
  nenhum componente usa hex direto. A web app do celular repete os mesmos
  tokens em `crates/core/assets/remote/style.css`.
- **Modelo por conversa:** fica na coluna `model` da tarefa. Nomes são
  validados antes de virar argumento da CLI (nada com espaço ou começando por `-`).
- **Troca de agente:** `Orchestrator::set_provider` guarda o estado do
  provedor que sai em `provider_state` e manda na frente do próximo turno um
  resumo de `agents/handoff.rs` (pedidos, respostas e arquivos alterados).
- **Modo de trabalho:** Isolado (git worktree) ou Direto na pasta, por tarefa.
  Pasta sem `.git` só aceita o Direto. No Isolado, nunca passe `--add-dir` da
  pasta original; os anexos ficam em `%APPDATA%\argus\anexos\` e cada conversa
  nova tem sua subpasta `novo\<id>\`.
- **Permissões:** `crates/core/src/permissions.rs`. No Windows o Claude Code
  usa a ferramenta `PowerShell`, não `Bash`: todo turno leva `--allowedTools`
  para as duas. "Git local" e "Git remoto" são níveis separados; push forçado,
  `reset --hard`, `clean` e shell genérico nunca são liberados por padrão.
- **Uso e custo:** números sempre em pt-BR (`src/api/util.ts`: `formatIntPt`,
  `formatTokensPt`, `formatUsdPt`). Modelos terminados em `-free`, `:free` ou
  `/free` custam 0, na mesma regra no core e no frontend. "Turnos" conta turnos
  distintos, não linhas de uso.
- **Acesso remoto:** servidor axum em `crates/core/src/remote.rs`, desligado
  por padrão. Todas as rotas exigem token por cookie HttpOnly `SameSite=Lax`
  ou `Authorization: Bearer`. Sem token, 401 inclusive no WebSocket. O pareamento
  troca `?token=…` por cookie e redireciona para `/`, tirando o token do endereço.
  CSP `default-src 'self'`; nada de recurso externo na web app.
- **Dados:** SQLite em `%APPDATA%\argus\` (`argus.db`, `pricing.json`,
  `allowed_tools.json`, `anexos\`). IDs de tarefa nunca são reaproveitados.

## Armadilhas conhecidas

- **Fotografar a janela:** `gdigrab -i "title=Argus Dev"` devolve quadro velho
  com WebView2 (o desenho da página não passa pelo DC da janela). Fotografe a
  tela inteira (`-i desktop`) e recorte pela moldura que a ponte informa. Não
  rode sessão de fotos com a máquina em uso: um recorte já pegou uma conversa
  real do usuário.
- **Conferir texto não é conferir navegação:** o roteiro compara `data-area`
  com a área esperada (`nav-<área>` na barra de ícones, `vazio` no `<Empty>`).
- **Galeria de estados:** uma fixture declara o que cada comando responde e
  cala eventos na mesma interceptação. Sem calar eventos, um `servers-changed`
  real apaga a lista fabricada segundos depois do boot.
- **Log do OpenCode:** gravado em `%APPDATA%\argus\logs\` com credenciais
  mascaradas. Linhas `timestamp=... level=...` vão só para o arquivo.
- **Worktree preso:** Aplicar/Descartar encerram processos ligados à pasta,
  tentam a remoção 4 vezes e só depois usam `--force --force` + `prune`.
- **Aviso da CLI do claude** "Warning: claude.ai MCP servers blocked" é
  filtrado no chat e no servidor remoto; não reintroduza esse texto na tela.

## Validação no aparelho

Quando houver celular conectado por USB com depuração, valide no aparelho via
Screen Mirror (`list_devices` → `ui_tree`/`screenshot` → ação → `screenshot`).
Teste unitário e leitura de código não substituem rodar o app de verdade. Se
uma ação for bloqueada pela política do Screen Mirror, informe o usuário e não
tente contornar por `adb` direto, outro app ou atalho.

O APK assinado é gerado por `gerar-apk.ps1` com keystore fora do repositório.
`gen/android` é regenerável: o script repõe o bloco de assinatura no
`build.gradle.kts` quando encontra `key.properties`.

## Convenções

- Texto de interface, mensagens de erro e documentação em pt-BR. Termos
  técnicos (comandos, APIs, nomes de arquivo) ficam como são.
- Sem emojis na interface e sem preâmbulo cerimonial na comunicação.
- Não refatore código fora do escopo do pedido. Uma correção descoberta no
  caminho vira pendência em `PROXIMOS_PASSOS.md`, não mudança silenciosa.
