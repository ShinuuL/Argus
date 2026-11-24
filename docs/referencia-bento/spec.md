# Especificação de Redesign UI & Prompt de Implementação — Argus IDE

Este documento detalha as decisões de layout, redistribuição funcional, design tokens e o prompt técnico pronto para você aplicar esse visual diretamente no código da sua IDE (React / Tailwind CSS / Electron ou Tauri).

---

## 1. Conceito Visual e Hierarquia

O objetivo do redesign é transformar uma interface densa com divisórias rígidas em um layout **Bento arredondado e minimalista**, reduzindo a poluição visual na área de composição de mensagens sem remover recursos avançados (fila de mensagens, seleção de modelo/esforço, contexto `@`, anexos e telemetria).

### Principais Mudanças Estruturais
1. **Superfícies Modulares (`Bento Panels`):**
   - A barra lateral de sessões e cada painel de agente aberto passam a ser contêineres independentes com cantos arredondados (`rounded-2xl` / `16px`), fundo elevado (`#16161A` sobre canvas `#0E0E11`) e borda sutil (`1px solid rgba(255, 255, 255, 0.07)`), eliminando linhas divisórias secas coladas nas extremidades da tela.
2. **Composer Minimalista em Pílula (`Pill Input`):**
   - Substituição da caixa multiline pesada com 7 controles visíveis por uma **cápsula única arredondada (`rounded-full` / `24px` quando expandida)** contendo apenas:
     - **Esquerda:** Botão `+` (gatilho de menu popover).
     - **Centro:** Input expansível com placeholder `"Digite / para comandos"`.
     - **Direita:** Botão de entrada por voz (microfone) ou botão circular de envio/parada (`Stop`) quando em execução.
3. **Barra de Metadados Discreta (Sub-input):**
   - Posicionada logo abaixo da pílula, alinhada à direita, contendo apenas três indicadores clicáveis de baixo contraste:
     - **Modelo e Esforço:** `Opus 5.5` (branco suave) + `Baixo` (cinza secundário) — abre popover rápido de modelo/thinking budget.
     - **Modo de Execução:** `Automático` (ou `Pedir aprovação` / `Plano`).
     - **Anel de Contexto/Status:** Indicador circular minimalista (ex.: anel fino mostrando % da janela de contexto usada ou estado ativo).
4. **Redistribuição das Funções Removidas da Barra:**
   - **Anexos, `@` (arquivos/MCPs), Prompts/Skills:** Movidos para o popover do botão `+` e acionáveis diretamente digitando `@` ou `/` no texto.
   - **Fila de mensagens (`entra na fila`):** Aparece dinamicamente como um pequeno badge/pill acima da pílula **somente** quando o agente está trabalhando (`trabalhando...`) e há itens enfileirados.
   - **Telemetria de tokens e custo (`96 entrada • US$ 2,29`):** Ocultada por padrão no fluxo principal e exibida sob demanda (no hover do turno ou no cabeçalho da aba).

---

## 2. Design Tokens (Cores, Raios e Espaçamento)

```css
:root {
  /* Fundos */
  --bg-canvas: #0d0d10;           /* Fundo base da janela */
  --bg-panel: #151519;            /* Painéis principais (sidebar e chats) */
  --bg-surface-elevated: #1e1e24; /* Pílula de input, cards e blocos de código */
  --bg-surface-hover: #26262e;    /* Hover em botões e itens de lista */

  /* Bordas */
  --border-subtle: rgba(255, 255, 255, 0.07);
  --border-input: rgba(255, 255, 255, 0.11);
  --border-focus: rgba(139, 92, 246, 0.45);

  /* Tipografia */
  --text-primary: #ececf1;
  --text-secondary: #9898a6;
  --text-muted: #636370;

  /* Acentos */
  --accent-primary: #7c6df2;
  --status-working: #60a5fa;
  --status-context-ring: #c2415c;

  /* Raios de Borda */
  --radius-panel: 16px;           /* rounded-2xl */
  --radius-card: 12px;            /* rounded-xl */
  --radius-pill: 9999px;          /* rounded-full */
}
```

---

## 3. Prompt para Aplicar no seu Projeto (Copie e Cole no seu Agente de Código)

```text
Você é um engenheiro frontend sênior especialista em Design Systems para IDEs modernas (estilo Linear, Zed e Cursor).

OBJETIVO:
Refatorar a interface do chat e dos painéis principais da nossa IDE (Argus) para adotar um visual escuro, minimalista e arredondado (estilo Bento), simplificando radicalmente a barra de entrada de mensagens sem perder funcionalidades.

ESPECIFICAÇÃO VISUAL E ESTRUTURAL:

1. LAYOUT GERAL E PAINÉIS ARREDONDADOS:
- Aplique um fundo base escuro (`#0D0D10`) com `gap` entre a sidebar de sessões e os painéis de chat ativos.
- Transforme a sidebar e cada coluna de chat em contêineres arredondados (`rounded-2xl`, fundo `#151519`, borda `1px solid rgba(255, 255, 255, 0.07)`).
- Arredonde os blocos de código e cards de resposta para `rounded-xl`, mantendo contraste suave (`#1A1B22`) e botão "Copiar" discreto no topo direito.
- Arredonde os itens ativos da lista de sessões com fundo translúcido suave (`rounded-lg`).

2. REDESIGN DO COMPOSER (BARRA DE CHAT RESUMIDA):
- Substitua a caixa atual de múltiplas linhas e ícones inferiores por um componente de input minimalista em formato de pílula (`rounded-full` quando em 1 linha, transicionando suavemente para `rounded-2xl` se o texto quebrar em múltiplas linhas).
- Fundo da pílula: `#1E1E24`, borda `1px solid rgba(255, 255, 255, 0.10)`, altura mínima `44px`.
- Estrutura interna da pílula (flex row):
  a) Esquerda: Botão circular `+` (`32px`) que abre um Popover Menu com as ações antes expostas: "Anexar arquivo/imagem", "Adicionar contexto (@)", "Usar prompt/skill" e "Ver fila de mensagens".
  b) Centro: Textarea auto-expansível com placeholder `"Digite / para comandos"`. Ao digitar `/`, abrir autocomplete de comandos rápidos (`/fila`, `/compactar`, `/limpar`, `/modelo`, `/mcp`). Ao digitar `@`, abrir seletor de arquivos/contexto.
  c) Direita: Botão de microfone quando vazio/inativo; botão de enviar quando houver texto; ou botão circular de Stop quando o agente estiver respondendo.

3. BARRA DE CONTROLES DISCRETA ABAIXO DA PÍLULA:
- Crie uma linha compacta logo abaixo do input (`mt-2 px-2 flex items-center justify-end gap-3 text-xs`), contendo apenas:
  - Seletor compacto de Modelo + Nível de Esforço: ex. `<span class="text-zinc-200 font-medium">Opus 5.5</span> <span class="text-zinc-500">Baixo</span>` (clicável para trocar modelo e nível de raciocínio).
  - Seletor compacto de Modo: ex. `Automático` (clicável para alternar entre Automático / Confirmar ações / Somente leitura).
  - Indicador circular de uso de contexto (`14px` SVG ring mostrando % da janela de contexto ocupada, com tooltip detalhado no hover).

4. LIMPEZA DE RUÍDO VISUAL:
- Remova textos fixos de atalho abaixo do input ("Enter põe na fila • Ctrl+Enter..."). Exiba atalhos apenas como tooltips ou dentro do menu `/`.
- Oculte as métricas detalhadas de tokens/custo abaixo de cada mensagem por padrão, exibindo-as apenas no hover da mensagem ou em um tooltip no badge de custo do cabeçalho da aba.
- Quando o agente estiver gerando resposta, mantenha um indicador sutil `••• trabalhando...` alinhado à esquerda logo acima da pílula.

TAREFA:
1. Localize os componentes responsáveis pelo layout dos painéis de chat, lista de sessões e barra de input (composer).
2. Refatore o CSS/Tailwind e a estrutura JSX/TSX seguindo fielmente os tokens e comportamentos acima.
3. Garanta que nenhuma funcionalidade existente (fila de mensagens, troca de modelo, anexos, MCPs, streaming e atalhos de teclado Enter / Ctrl+Enter) seja perdida ao mover os controles para o menu `+` e comandos `/`.
```