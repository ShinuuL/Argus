# Feat: espelhamento como aba, agentes/skills globais e editor de fluxo

Especificação de três entregas que se apoiam uma na outra:

1. **Espelhamento (Screen-Mirror) como aba do workbench**, e não só como área em tela cheia.
2. **Agentes e skills gravados em formato global**, com frontmatter, para qualquer CLI ler.
3. **Editor visual de fluxo de agentes**, com orquestrador e modelo por agente, que grava
   os arquivos do item 2.

Estado atual relevante:

- `src/store/workbench.ts` — abas (`TabKind`) e grupos; `PAGE_AREAS` desvia `devices` para a
  área em tela cheia `devices`.
- `src/views/devices/index.tsx` — painel do Screen-Mirror embutido num iframe, com faixa de
  estado e lista de aparelhos; `crates/core/src/devices.rs` e `src-tauri/src/commands/devices.rs`
  sobem e monitoram o processo auxiliar.
- `src/views/agentmap/` e `src/areas/agents/` — visualização **somente leitura** do que já
  rodou. O editor de fluxo é a contraparte de escrita, que hoje não existe.

---

## 1. Espelhamento como aba

### Problema

`openPage("devices")` cai em `PAGE_AREAS` e troca a área inteira. O usuário perde a conversa
de vista justamente quando mais precisa dos dois lados juntos: agente mexendo no código e tela
do celular reagindo.

### Mudança

Uma aba própria, com o aparelho na identidade da aba — dois celulares conectados abrem duas
abas, e `sameKind` as distingue:

```ts
// src/store/workbench.ts
/** Espelhamento de um aparelho Android. `serial` null = aparelho único/padrão. */
| { type: "mirror"; serial: string | null }
```

- `sameKind`: `a.serial === b.serial`.
- `tabTitle`: `serial ? \`Celular ${serial}\` : "Celular"`.
- `tabTooltip`: modelo do aparelho quando `DeviceList` já carregou.
- `openMirror(serial?: string | null)` no store, espelhando `openAgentMap`.
- `devices` **sai** de `PAGE_AREAS`? Não — a área cheia continua, é o lugar de gerenciar
  aparelhos, apps, arquivos e logcat. A aba é a visão focada: só a tela espelhada e os
  controles essenciais.

### Componente

`src/views/mirror/index.tsx` — novo, sem conflito com `src/views/devices/`:

- Reaproveita `api.devicesStatus()` / `api.devicesStart()` e a função `missing()` (extrair de
  `views/devices/index.tsx` para `views/devices/status.ts`, único ponto de contato com o
  arquivo existente — fazer depois que o outro agente soltar a área).
- Iframe apontando para a rota de espelhamento do painel, com `?serial=` e sem a navegação
  interna do Screen-Mirror (a aba é enxuta).
- Barra inferior: Voltar / Início / Recentes, girar, captura de tela, e um botão
  "Abrir Dispositivos" para a área cheia.
- Sem aparelho conectado: estado vazio com o mesmo texto de diagnóstico de `missing()`.

### Como se abre

- Botão "Abrir como aba" na faixa da área Dispositivos.
- Paleta de comandos: "Espelhar celular".
- Clique no aparelho na lista de `DeviceList`.
- Quando o agente pedir uma ação no celular via Screen-Mirror e nenhuma aba de espelhamento
  estiver aberta, abrir em pré-visualização (`preview: true`) — o usuário vê o que o agente
  está fazendo sem precisar procurar.

### Ciclo de vida

O processo auxiliar já é compartilhado; a aba **não** o encerra ao fechar. Encerrar continua
sendo ação explícita da área Dispositivos. Várias abas apontando para o mesmo serial são o
mesmo iframe lógico — sem custo extra relevante.

---

## 2. Agentes e skills globais, com frontmatter

### Objetivo

O que o Argus grava tem que ser lido por Claude Code, Codex, Gemini CLI e afins, sem formato
proprietário. Isso significa arquivos Markdown com frontmatter YAML nos caminhos que essas
CLIs já procuram.

### Formatos

**Agente** — `.claude/agents/<slug>.md` (projeto) ou `~/.claude/agents/<slug>.md` (global):

```markdown
---
name: revisor-fiscal
description: Revisa emissão de NF-e e CF-e contra a legislação vigente. Use quando a mudança tocar impostos, SEFAZ ou TEF.
model: sonnet
tools: Read, Grep, Glob, Bash
---

Você revisa alterações fiscais do ERP...
```

Campos: `name` (slug, obrigatório), `description` (frase de acionamento, obrigatória — é o que
decide o roteamento), `model` (`opus` | `sonnet` | `haiku` | `fable` | `inherit`), `tools`
(lista; omitido = todas), e `color` opcional para a UI.

**Skill** — `.claude/skills/<slug>/SKILL.md`:

```markdown
---
name: backup-firebird
description: Backup e restauração do banco Firebird do ERP. Use ao falar de gbak, backup, restore ou .fbk.
---

## Quando usar
...
```

**AGENTS.md** — na raiz do projeto, o arquivo-contrato que quase toda CLI lê. O Argus gera e
mantém a seção dele que descreve o fluxo, delimitada por marcadores, preservando o que o
usuário escreveu fora deles:

```markdown
<!-- argus:inicio fluxo -->
...gerado...
<!-- argus:fim fluxo -->
```

### Escopo global vs. projeto

Um seletor **Global / Este projeto** na criação. Global grava em `~/.claude/...` e vale para
qualquer pasta; projeto grava em `.claude/...` e entra no git. Compatibilidade: quando o
diretório do fornecedor diferir (ex.: `~/.codex/`), gravar o arquivo canônico em `~/.claude/`
e criar link simbólico nos demais — no Windows, cópia, com reescrita a cada salvamento.

### Backend

Novo módulo `crates/core/src/agentdefs.rs`:

- `list_agent_defs(scope)` → lê e faz o parse do frontmatter dos dois escopos, marcando
  conflitos (mesmo `name` em global e projeto: projeto vence).
- `save_agent_def(def, scope)` → serializa frontmatter + corpo, escrita atômica
  (arquivo temporário + rename), nunca sobrescreve sem ler antes.
- `delete_agent_def(name, scope)`.
- Mesmas três para skills, e `sync_agents_md(project)` para o bloco delimitado.

Parser de frontmatter: aceitar só o subconjunto YAML que importa (escalares e listas simples),
para não arrastar dependência pesada nem aceitar YAML arbitrário de terceiros.

---

## 3. Editor visual de fluxo de agentes

### Conceito

Um **fluxo** é um orquestrador mais os agentes que ele chama, com as ligações entre eles. O
usuário desenha; o Argus grava os `.md` com frontmatter do item 2 mais um manifesto do
desenho.

### Tela

Nova aba `{ type: "flow"; flow: string }` e a tela `src/views/flow/`:

- **Tela (canvas)** ao centro: nós arrastáveis, ligações ortogonais. Reaproveitar o layout de
  `src/views/agentmap/graph.ts`, que já posiciona árvore de agentes — a diferença é que aqui
  o grafo é editável e vem do manifesto, não dos eventos.
- **Paleta** à esquerda: agentes já existentes (global e do projeto) para arrastar, e
  "Novo agente".
- **Inspetor** à direita, para o nó selecionado: nome, descrição (o campo que mais importa —
  é a função do agente e o gatilho de acionamento), modelo, ferramentas permitidas, escopo e
  o prompt do corpo.

### Tipos de nó

| Nó | Papel |
|---|---|
| **Orquestrador** | Raiz, exatamente um. Decide a quem delegar. Vira `AGENTS.md` + um agente `orquestrador`. |
| **Agente** | Executa uma função. Vira `.claude/agents/<slug>.md`. |
| **Paralelo** | Marca filhos que rodam juntos. Vira instrução no prompt do orquestrador — não há arquivo próprio. |
| **Condição** | Bifurcação por critério em texto. Idem: instrução no prompt. |

Paralelo e condição não existem como arquivo porque nenhuma CLI os lê; existem como texto
explícito no prompt do orquestrador, o que qualquer CLI entende. Deixar isso claro na UI, para
o desenho não prometer mais determinismo do que entrega.

### Modelo por agente

Seletor por nó, alimentado pelos provedores já configurados (área Configurações › Provedores),
com "herdar do orquestrador" como padrão. Mostrar no nó, no canto, a etiqueta do modelo — a
leitura de custo do fluxo tem que ser imediata.

### Geração

Botão **Gerar agentes**, com pré-visualização do diff antes de escrever:

1. Um `.md` por nó de agente, com frontmatter + corpo.
2. `AGENTS.md` com o bloco delimitado descrevendo o fluxo em prosa.
3. `.claude/argus/fluxos/<slug>.json` — manifesto do desenho (posições, ligações, tipos de nó),
   para reabrir o editor. É o único arquivo proprietário, e é deliberadamente só sobre o
   desenho: apagá-lo não quebra nenhum agente.

A volta também vale: abrir um projeto sem manifesto e com `.claude/agents/` populado monta um
fluxo plano (orquestrador + todos os agentes), que o usuário rearranja.

---

## Ordem sugerida

1. **Aba de espelhamento** — isolada, valor imediato, só encosta em `workbench.ts` e um
   arquivo novo.
2. **`agentdefs.rs` + leitura/escrita com frontmatter** — base do resto; entregável sozinho
   como uma seção "Agentes" em Configurações, só com lista e edição em formulário.
3. **Editor de fluxo** — sobre o 2, reaproveitando o layout de `agentmap/graph.ts`.

## Arquivos em disputa

`src/shell/NavRail.tsx`, `src/shell/icons.tsx`, `src/views/chat/Composer.tsx` e
`src/views/chat/slash.ts` estão modificados por outro trabalho em andamento. Os pontos de
entrada da interface (ícone na barra, comandos de barra) devem ser os **últimos** a serem
tocados, depois que essas mudanças assentarem.
