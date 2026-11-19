# Usar o Argus como o Claude Code no VS Code

_Atualizado em 2026-11-19 (versão 0.1.2, claude CLI 2.1.220)._

**Resposta curta:** sim, para o trabalho do dia a dia. O Argus roda a mesma CLI oficial do
claude, com o seu login e a sua assinatura, as suas configurações (`~/.claude`), os seus MCPs
e o `CLAUDE.md` dos projetos. O que muda é a forma de aprovar ações: no `claude -p` não há
pergunta no meio do turno, então o Argus decide antes (modos e comandos liberados) e oferece
liberar depois. Abaixo, o que foi conferido numa janela real, com tarefas reais, e o que não é
igual.

## Equivalências conferidas

| No Claude Code (VS Code) | No Argus | Conferido |
|---|---|---|
| Conversa com o agente, resposta aparecendo enquanto é escrita | Aba de conversa; texto ao vivo (`--include-partial-messages`) | sim |
| Escolher o modelo | Pílula "agente · modelo" no campo de mensagem | sim |
| Modo de planejar | **Planejar**: só leitura, termina com o plano; "Executar o plano" volta ao Normal e executa | sim (haiku, US$ 0,07) |
| Aceitar edições automaticamente | **Normal** (padrão): edita sem perguntar; shell só com comandos liberados | sim |
| Bypass de permissões | **Bypass**, só na conversa, com confirmação e selo vermelho | sim |
| Escolher o modo antes de começar | Opções da conversa, já na conversa nova | sim |
| Mencionar arquivo / seleção (`@arquivo#L1-10`) | Alt+K no editor; chip "arquivo em foco"; "@" no campo | sim |
| Ver os erros do editor | Chip "N erros do editor" (erros de sintaxe do Monaco) | sim |
| Diff das alterações, rejeitar um trecho | Inspetor › Alterações e aba de diff com "Rejeitar trecho" / "Rejeitar arquivo" | sim (inclusive CRLF) |
| Checkpoints ("Rewind") | **Voltar a este ponto** em cada pedido: a pasta volta ao estado de antes dele | sim |
| Pergunta com opções do agente | Cartão clicável (`argus-ask`), a escolha vira a próxima mensagem | sim |
| Anexar imagem / arquivo | Clipe ou colar; a pasta do anexo é liberada para o agente | sim |
| `/compact` | Compactação automática ao passar do limite (padrão 200 mil tokens) | sim |
| Retomar conversa | As conversas ficam na barra lateral e continuam com `--resume` | sim |
| Cancelar o turno | Botão Parar; o uso parcial é registrado | sim |

## O que o Argus tem a mais

- Modo **Isolado**: o agente trabalha numa cópia (git worktree); você revisa e aplica ou descarta.
- Várias conversas e vários projetos ao mesmo tempo, com o estado de cada uma na barra lateral.
- Auto-work: resolve as pendências do `PROXIMOS_PASSOS.md` uma por turno, com limites de custo.
- Tarefas agendadas, métricas de consumo, servidores de dev, celular espelhado e acesso remoto.
- Outros agentes na mesma conversa (OpenCode, Codex, Gemini), com troca e resumo automático.

## O que não é igual

1. **Aprovação no meio do turno.** No VS Code a extensão pergunta "permitir este comando?". No
   Argus o comando fora da lista é negado, o turno segue, e aparece "Comando negado" com
   **Liberar este comando neste projeto** e **Liberar e repetir**. No modo Auto (padrão das
   permissões) quase todo shell é liberado e só os comandos nocivos são negados.
2. **O claude nega `cd <pasta>` seguido de `git` no mesmo comando** (proteção da CLI). O aviso
   de sistema orienta o agente a usar `git -C pasta`.
3. **Planejar não é o `--permission-mode plan` da CLI.** Esse modo troca o modelo escolhido
   pelo sonnet por conta própria; o Planejar do Argus nega as ferramentas de escrita e mantém o
   modelo. O resultado é o mesmo (plano sem alterar nada), mais barato.
4. **O editor é o Monaco, não o VS Code.** Sem extensões nem checagem de tipos completa: o chip
   de erros vê erros de sintaxe. Para depurar ou usar extensões, abra o projeto no VS Code ao
   lado; o Argus acompanha as alterações da pasta.
5. **Custo da primeira mensagem.** Cada conversa nova grava o contexto no cache (cerca de 35 mil
   tokens). Perguntas avulsas saem mais baratas numa conversa já aberta.
6. **Codex e Gemini** não foram conferidos neste computador (CLIs não instaladas).

## Garantia

O que está marcado "sim" na tabela foi usado numa janela real em 2026-11-19, com o claude
2.1.220, e os testes automáticos (230 no núcleo) passam. Uma versão nova da CLI pode mudar
flags e formato da saída; o Argus detecta as flags pelo `claude --help` e cai num modo
compatível quando alguma falta, mas cada atualização grande da CLI merece uma conferência
rápida: uma conversa, um Planejar e um "Voltar a este ponto".
