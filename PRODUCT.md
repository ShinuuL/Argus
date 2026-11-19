# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

Um único usuário: o próprio desenvolvedor que construiu o Argus e o usa todo dia como ferramenta
de trabalho. Não há segundo público — nem time, nem cliente, nem usuário externo. Os releases com
instalador existem para instalar o app na própria máquina, não para distribuir a terceiros.

A situação de uso: várias tarefas de programação acontecendo ao mesmo tempo, em projetos
diferentes, delegadas a agentes de IA que trabalham sozinhos por minutos. O trabalho real do
usuário não é escrever o código linha a linha; é **despachar, acompanhar e revisar** vários
agentes em paralelo — saber de relance quem está trabalhando, quem terminou, quem errou e quem
está esperando uma resposta, e revisar o que cada um mudou antes de aplicar.

Como é uma pessoa só, a interface pode ser densa e assumir conhecimento: o usuário conhece git,
worktrees, as CLIs dos agentes e a própria estrutura de pastas. Nada precisa ser explicado do
zero, e não existe primeira execução para desconhecido.

## Product Purpose

IDE desktop que **orquestra agentes de IA de várias CLIs** (Claude Code, OpenCode, Codex e Gemini)
sobre vários projetos ao mesmo tempo, e mantém o controle do que cada um fez.

O ciclo que o produto serve: abrir uma pasta raiz, ver todos os projetos dela, começar uma
conversa num projeto, deixar o agente trabalhar numa cópia isolada, acompanhar passo a passo o que
ele leu e editou, revisar o diff e então aplicar ou descartar. Mais o que cerca isso: terminais,
servidores de desenvolvimento, métricas de uso e custo, builds, git e acompanhamento pelo celular.

Sucesso é o usuário conseguir manter vários agentes produzindo sem perder o fio: sem precisar de
um terminal por agente, sem perder histórico ao trocar de agente e sem aplicar no projeto uma
mudança que ele não revisou.

## Positioning

Duas coisas que as alternativas (a Orca, o painel do Claude Code no VS Code, vários terminais
abertos) não fazem:

1. **Controla a pasta acima de todos os projetos**, não um projeto por janela. A unidade de
   trabalho é a pasta raiz: todos os projetos dentro dela aparecem na mesma barra lateral, com
   suas sessões, branches, servidores e alterações, e as ações atravessam projetos.
2. **Memória ligada ao vault do Obsidian** — o Argus lê e escreve o conhecimento do trabalho no
   vault em vez de guardá-lo só no banco do app. Hoje a pasta raiz aberta é justamente a pasta do
   vault. **Ainda não implementado:** não existe nenhum código de Obsidian ou de vault no
   repositório; é direção de produto confirmada, não funcionalidade.

Em volta disso, o que o produto já faz e os vizinhos não: troca de agente no meio da tarefa
preservando histórico e pasta (o agente que entra recebe um resumo montado pelo Argus), modo
Isolado por git worktree com Aplicar/Descartar, política de permissão própria sobre o shell dos
agentes, e a mesma conversa acompanhável pelo celular.

## Operating Context

- **Windows primeiro.** Janela Tauri 2 com WebView2; o shell dos agentes no Windows é PowerShell,
  não Bash, e a política de permissões trata os dois.
- **Interface em português do Brasil**, inclusive mensagens de erro e rótulos.
- **Pasta raiz** com os projetos como subpastas de primeiro nível; hoje essa raiz é a pasta do
  vault do Obsidian. Os worktrees dos agentes ficam em `<projeto>/../.argus-worktrees/`.
- **Dados em `%APPDATA%\argus\`**: `argus.db` (SQLite), `pricing.json`, `allowed_tools.json` e
  `anexos\` (apagados depois de 1 dia). A versão de desenvolvimento usa `%APPDATA%\argus-dev\`.
- **Tela larga**, de ~1100 px para cima; sessões longas, muitas abas e split de até 3 grupos.
- **Acompanhamento pelo celular:** servidor axum desligado por padrão, pareamento por token com
  cookie HttpOnly, web app em HTML/CSS/JS puros (sem framework e sem CDN). Quando exposto, é por
  túnel (Tailscale Funnel, cloudflared, ngrok) e só enquanto o servidor estiver ligado.
- **Agentes rodam pelas CLIs oficiais**, com o login da assinatura de cada uma.

## Capabilities and Constraints

Confirmado e em uso:

- Vários projetos de uma pasta raiz, cada um com suas sessões; lista que se atualiza sozinha.
- Conversa com agente: stream de eventos, ferramentas em linhas recolhidas, raciocínio, fila de
  mensagens, anexos, rascunho por sessão, menções `@arquivo`/`@agente` e comandos de barra.
- **Modo de trabalho por tarefa:** Isolado (git worktree, com Aplicar/Descartar) ou Direto na
  pasta (diff contra o último commit, sem aplicar). Pasta sem `.git` só aceita o Direto.
- **Troca de agente e de modelo** por conversa, com o estado de cada provedor preservado.
- Diff revisável com comentário por linha que volta para o rascunho da conversa.
- Terminais (painel e como aba), servidores de desenvolvimento com detecção de túnel, builds,
  métricas de uso e custo, área de Git (PRs, repositórios, branches, alterações) e dispositivos
  Android por sidecar.
- Quatro temas (Oceano, Ardósia, Grafite e Noite), escolhidos em Configurações; valem também na web app.

Restrições duráveis, que trabalho futuro não pode quebrar:

- **Nada de chave de API e a IDE nunca lê nem guarda tokens.** Os agentes autenticam pelas
  próprias CLIs.
- **Shell do agente bloqueado por padrão**, exceto o conjunto de leitura/inspeção, os níveis "Git
  local" e "Git remoto" quando ligados e o que estiver liberado por projeto. Nunca shell genérico.
  No modo Auto, os comandos nocivos continuam bloqueados.
- **Nunca `--add-dir` da pasta original** do projeto numa sessão isolada.
- **Custo sempre rotulado "equivalente em API"**, nunca como cobrança real.
- **Nenhuma marca ou nome de empresa** em lugar nenhum do projeto ou da interface. Identificador
  do app: `dev.argus.app`.
- **Toda cor vem de `src/styles/tokens.css`**, nos três temas; nenhum componente usa hex direto.
  O Monaco e o xterm montam o tema a partir dos mesmos tokens.
- Erro nunca depende só de cor: vem com ícone ou rótulo junto.
- A versão fica em cinco arquivos que precisam andar juntos (`package.json`, `src-tauri`, o
  `Cargo.toml` do workspace e os dois do `src-mobile`).

Explicitamente indeciso:

- Como a memória no vault do Obsidian funciona (formato, o que é gravado, quando) — só a direção
  está decidida.
- Se os projetos absorvidos como módulos (espelhamento de celular, gerenciador de portas, captura
  de rede) continuam existindo como ferramentas separadas depois da integração.

## Brand Commitments

- Nome do produto: **Argus**. Sem marca de empresa, sem logotipo de terceiros e sem atribuição na
  interface — quando algo de fora é usado, o aviso fica em arquivo, não na tela.
- Voz dos textos: português do Brasil, direto e técnico, dizendo o que a coisa faz. Erro nomeia o
  problema e a saída (ex.: "Liberar este comando neste projeto").
- O ícone atual do app (um olho) é provisório e será redesenhado; conceitos em `docs/icone/`.
- A Orca (<https://github.com/stablyai/orca>, MIT) e o painel do Claude Code no VS Code são
  referência de **padrões** — medidas, hierarquia e comportamento —, não de código copiado nem de
  marca. Nada do nome, do logotipo ou das ilustrações dela entra aqui.

## Evidence on Hand

- `PROXIMOS_PASSOS.md`: estado real, funcionalidade por funcionalidade, com a distinção entre
  "visto na janela", "compila" e "core testado". É a fonte sobre o que de fato foi exercitado.
- `PENDENCIAS.md`: plano de absorver as outras três ferramentas de desenvolvimento como módulos.
- `docs/PLANO_UI_ORCA.md`: pesquisa e plano de aproximação da Orca, fases 1–4 e 6–9 entregues em
  2026-11-16; a fase 5 (ícone do app) segue pendente.
- `docs/mockups/argus-telas.html`: mockup de 12 telas nos três temas, aprovado.
- `docs/prints/`: prints de validação. `docs/icone/`: conceitos de ícone.
- Testes: 141 no core + 18 no desktop; `npx tsc --noEmit` para a interface. A interface **não** tem
  runner de testes próprio.
- Validação na janela por protocolo de depuração do WebView2 (CDP), com tarefas reais do **claude**
  e do **opencode**.

O que não existe e não pode ser afirmado: **codex e gemini nunca foram exercitados** (não estão
instalados na máquina de teste); o arrasto vindo do Explorer do Windows e a área de transferência
real do sistema não são cobertos pelos testes; não há usuário externo, depoimento, métrica de
adoção, benchmark nem licença declarada no repositório.

## Product Principles

1. **A raiz é a unidade, não o projeto.** Qualquer tela nova deve funcionar com vários projetos ao
   mesmo tempo, não assumir um só.
2. **Nada entra no projeto sem revisão.** O caminho padrão é trabalho isolado, diff visível e uma
   decisão explícita de aplicar ou descartar.
3. **O estado de cada agente se lê de longe.** Quem trabalha, quem terminou, quem falhou e quem
   espera resposta precisa ser distinguível sem abrir a sessão e sem depender só de cor.
4. **A confiança é do usuário, não do agente.** Permissão é lista fechada e auditável; o que o
   agente pode rodar é decisão registrada, nunca um padrão permissivo silencioso.
5. **Um usuário que sabe o que faz.** Densidade e atalhos valem mais que explicação; o produto não
   gasta tela ensinando o que essa pessoa já sabe.
