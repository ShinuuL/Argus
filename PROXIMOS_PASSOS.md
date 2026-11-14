# Argus — Estado atual e próximos passos

_Atualizado em 2026-11-14._

O Argus é uma IDE desktop em Rust (Dioxus 0.7) que gerencia vários projetos de uma pasta raiz e orquestra agentes de IA (Claude Code e OpenCode). Os agentes usam o login das próprias CLIs oficiais; a IDE nunca lê nem guarda tokens.

## Como rodar

```powershell
cargo run -p argus-app            # abre a IDE
cargo test --workspace            # 57 testes core + 8 app
```

Variáveis úteis para testes:

| Variável | O que faz |
|---|---|
| `ARGUS_ROOT` | abre essa pasta raiz automaticamente |
| `ARGUS_DB` | usa outro banco SQLite |
| `ARGUS_ALLOWED_TOOLS` | usa outro arquivo de comandos permitidos |

Os dados ficam em `%APPDATA%\argus\`: `argus.db`, `pricing.json` e `allowed_tools.json`. O histórico antigo da pasta `harness` foi copiado para lá.

## Ao renomear a pasta do projeto

- Nenhum caminho absoluto está no código, então nada quebra.
- A pasta `target/` vai recompilar do zero na primeira vez, o que leva alguns minutos.
- Os worktrees dos agentes ficam em `<projeto>/../.argus-worktrees/`. Worktrees antigos em `.harness-worktrees` podem ser apagados com `git worktree prune` no projeto de origem.
- O repositório git ainda não tem nenhum commit. Recomendo fazer o primeiro commit logo depois de renomear.

## Estrutura

```
crates/core   (argus-core): workspace, monitor, pty, procs, deps, metrics, db, remote, autowork,
              agents/{provider, claude, opencode, orchestrator}
crates/app    (argus-app):  main.rs, state.rs, ui/{workbench, sidebar, editor, terminal, agents,
              autowork, servers, deps, metrics, remote, player, pet}.rs
              assets/{monaco, xterm, remote (web app do celular)}
```

## Funcionalidades

| Recurso | Estado |
|---|---|
| Explorador multiprojeto + status git ao vivo | ✓ testado na tela |
| Editor Monaco, abas estilo VS Code, split, arrastar, Ctrl+Tab/W/\\ | ✓ testado na tela |
| Terminais embutidos (ConPTY) | ✓ testado na tela |
| Provedores claude/opencode, login pela CLI, status | ✓ testado na tela |
| Tarefas em worktree isolado, stream, diff, aplicar/descartar, responder (`--resume`) | ✓ tarefa real |
| Histórico SQLite persistente | ✓ |
| Métricas de tokens e custo equivalente em API | ✓ tarefa real |
| Dependências: detecção e instalação com confirmação, comando inexistente no terminal | ✓ testado na tela |
| Acesso remoto pela LAN (QR + token) | ✓ API testada; **web app nunca aberta no celular** |
| Auto-work (PENDENCIAS.md, TODO/FIXME, build/testes, travas) | ✓ tarefa real concluída 2/2 |
| Player de música (mídia do Windows + arquivos locais) | ✓ testado na tela |
| Pet de uso | ✓ visível; **estado "feliz" não confirmado** |
| Servidores vinculados à sessão + painel lateral | ⚠ implementado e com testes, **nunca visto funcionando** |
| Visão somente leitura dos subagentes | ✗ **não implementado**: o agente da rodada 4 recusou a tarefa |

## Próximos passos (em ordem)

1. **Visão dos subagentes (somente leitura).**
   - Capturar uma saída real do `claude -p --output-format stream-json --verbose` com uso da ferramenta Task/Agent e guardá-la como fixture.
   - No parse, as mensagens dos subagentes vêm com `parent_tool_use_id`.
   - Criar os eventos `SubagentStart`/`SubagentEnd`, persistir no banco e montar a árvore na aba da sessão.
   - O chat do subagente é só visualização: sem caixa de resposta, com o aviso "somente leitura".
   - No chat principal, mostrar o cartão "→ subagente X".
   - Ter a mesma visão no remoto.
2. **Teste visual dos servidores.**
   - Subir `python -m http.server 8799` num terminal da IDE e confirmar que aparece no painel; derrubá-lo e confirmar que a porta fechou.
   - Testar o vínculo a uma sessão liberando `python -m http.server` em "Comandos permitidos".
3. **Abrir a web app no celular.** Na primeira vez, o Windows pede permissão no Firewall: clique em **Permitir** (rede privada).
4. **Pet feliz:** aumentar a janela do estado "feliz" de 10 s para cerca de 60 s, ou até o usuário interagir.
5. **Pequenos ajustes conhecidos:**
   - A caixa "derrubar servidores?" aparece também quando você só quer parar o turno. Deveria ter a opção "só parar o turno".
   - O botão "Reiniciar" de um servidor roda sem saída visível. Seria melhor reabrir o processo num terminal da IDE.
   - Trocar o `netstat` por GetExtendedTcpTable, o que exige adicionar a crate `windows` ao core.
   - Espaçar as verificações de build e testes do auto-work em projetos grandes, ou torná-las opcionais.
   - Um bloco de texto seguido de chamada de ferramenta: conferir se tudo aparece no log.
   - O OpenCode não informa o modelo usado; o custo cai na tabela de fallback.
6. **Empacotamento:** `cargo install dioxus-cli`, depois `dx bundle` para gerar o instalador, copiando `assets/` junto do executável.
7. **Ícone e identidade visual do Argus.** Sugestão: um olho; o pet pode ganhar um olho que acompanha o que acontece.
8. **Uso de CPU e GPU por sessão.**
   - Mostrar, em cada sessão, o consumo de CPU, memória e GPU dos processos dela (agente, subprocessos e servidores vinculados).
   - A árvore de processos por sessão já existe em `crates/core/src/procs.rs`; a CPU e a memória vêm do `sysinfo`.
   - GPU por processo no Windows: contadores de desempenho "GPU Engine" (PDH) ou `nvidia-smi` quando houver placa NVIDIA.
9. **Conexões com serviços de desenvolvimento.**
   - GitHub, Google Cloud, Firebase, Vercel e outros.
   - Seguir a mesma regra dos agentes: usar a CLI oficial de cada serviço (`gh`, `gcloud`, `firebase`, `vercel`), que faz o próprio login. A IDE não lê nem guarda tokens.
   - Tela "Conexões": detectar a CLI, mostrar o status de login, botão "Fazer login" no terminal embutido e ações por projeto (PRs e issues, deploys, logs).

## Decisões fixas

- Os agentes são as CLIs oficiais, com o login da assinatura. Nada de chave de API, e a IDE não lê tokens.
- O claude roda com `--permission-mode acceptEdits`. O Bash fica bloqueado, a não ser o que estiver liberado em "Comandos permitidos" por projeto.
- Nenhuma marca ou nome de empresa no projeto.
- O custo é sempre exibido como "equivalente em API".
