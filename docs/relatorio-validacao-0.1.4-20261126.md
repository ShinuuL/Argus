# Argus 0.1.4 - validação em sessão real (26/11/2026)

Diagnóstico só de leitura: sem mudança de código, sem commit, sem reinstalação.
O item 3 está sendo testado em outra sessão.

## Ambiente

- `claude auth status`: `loggedIn: true` (claude.ai, firstParty).
- `argus-desktop.exe`: ProductVersion/FileVersion `0.1.4`. Não há `resources\app\package.json`.
- Logs: `C:\Users\MASTERSYS\AppData\Roaming\argus\logs\`
- MCP no início da sessão: playwright `CONNECT_TIMEOUT 30000ms` (conectou depois); screen-mirror, aseprite e
  digitama `CONNECTION_CLOSED`.

## Resultado

| Item | Evidência | Resultado |
|---|---|---|
| 1. Dois turnos no mesmo host | Task 23: turnos 1 e 2 no host `claude.exe` PID 13544 (pai `argus-desktop` 2580, criado 09:42:42). No turno 3 o host passou a ser o PID 11528 (criado 09:55:32), com `host_ms=84` e `system init` novo às 12:55:57Z, sem nenhuma linha `cancel`/`out_stale_no_ack` explicando o descarte. Task 22: o turno 2 entrou no host do turno 1 (`command_lifecycle 27cd5b3a… queued/started` às 12:23:42.531Z no log `claude-22-1795695545774.log`). | ok (turnos 1→2); inconclusivo (troca no turno 3; possível causa: permissão alterada entre os turnos) |
| 2. Servidor em segundo plano | `Start-Process python -m http.server 4199 -WindowStyle Hidden` → PID 16952. Depois do fim do turno: `curl.exe` → `http 200`; `netstat`: `127.0.0.1:4199 LISTENING 16952`. | ok |
| 3. Cancelamento preserva o host | Task 24 (`claude-24-1795698243584.log`), pedido "conta de 1 a 60": `13:04:39.730Z task_started (local_bash)` → `13:04:39.747Z meta cancel: interrompendo o comando deste turno` → `13:04:44.761Z meta cancel sem ack da CLI (timeout): host sera descartado` → `meta cancel sem confirmacao: host descartado`. O usuário precisou clicar 3 vezes em "Parar"; só o primeiro clique aparece no log. | falha (bug E) |
| 4. Terminal deixado aberto | Uma janela do Windows Terminal (`WindowsTerminal.exe -Embedding`, título "claude") abre a cada spawn de host: WT 19604 criado 09:19:47 (spawn da task 22 às 12:19:47Z); WT 21692 criado 09:55:33, 1 s depois do host 11528. O host tem um filho `conhost.exe 0x4` sem `--headless` (ex.: 7552 do PID 13544). O usuário confirmou: abre um terminal por sessão; fechou só o primeiro. | falha (bug B) |
| 5. Lentidão no start | Ver tabela `lat` abaixo. O primeiro evento leva 44-51 s em host novo e 7,7 s em host reaproveitado. | falha: domina `prepare_ms` + init do host novo |
| 6. `<SessionUsage>` no cabeçalho | Print do cabeçalho: `CPU 6% · 369 MB`, `GPU 0% · 7 proc.`, sem erro. | ok |
| 7. Celular | screen-mirror `CONNECTION_CLOSED`; não foi possível listar aparelhos. | inconclusivo |

## Linhas `lat` brutas

| Log | Turno | prepare_ms | host_ms | first_event_ms | result_ms | cmd |
|---|---|---|---|---|---|---|
| claude-21-1795694610039 | 1 (haiku) | 10 | 75 | 4474 | 24867 | novo |
| claude-22-1795695545774 | 1 | 41734 | 60 | 51392 | 166580 | novo |
| claude-22-1795695776166 | 2 | 46316 | — | 46365 | 419398 | `--resume` (host reaproveitado) |
| claude-22-1795696952249 | 3 | 206576 | — | 206580 | 207144 | `--resume`; dois `out_stale_no_ack` às 12:45:59Z |
| claude-23-1795696937826 | 1 | 24309 | 115 | 44121 | 350932 | novo, host 13544 |
| claude-23-1795697619562 | 2 | 6574 | — | 7696 | 51074 | `--resume`, host 13544 reaproveitado |
| claude-23-1795697727456 | 3 | 4568 | 84 | 23149 | — | `--resume`, host novo 11528 |

`mcp_ms=0` em todos.

## Pendências (bugs)

- **A. Log do turno no arquivo errado.** Com o host reaproveitado, a linha `cmd` do turno novo é gravada com
  `--resume` num arquivo novo, mas os eventos do turno continuam indo para o log do primeiro turno do host.
  Reprodução: dois turnos seguidos na mesma conversa; comparar os arquivos `claude-<task>-*.log`.
- **B. Uma janela de terminal por host.** Cada spawn do `claude.exe` abre um Windows Terminal (`-Embedding`) visível,
  que fica aberto depois de o host terminar. Reprodução: Windows 11 com o Windows Terminal como terminal padrão;
  abrir uma conversa nova no Argus.
- **C. `prepare_ms` alto.** De 24 a 46 s nos primeiros turnos e 206 s no turno 3 da task 22, logo seguido de dois
  `out_stale_no_ack` com `result` antigos. Parece esperar o host anterior, que tinha subagentes em segundo plano
  (encerrados às 12:42:52Z). Reprodução: mandar um turno enquanto os subagentes do turno anterior ainda rodam.
- **D. Host descartado sem registro.** Task 23, turno 3: host trocado (13544 → 11528) sem linha de cancelamento,
  ack ou descarte no log. Conferir se é a mudança de permissão; se for por design, registrar o motivo no log.

- **E. Cancelamento sem ack e "Parar" sem efeito visível.** Com um Bash em primeiro plano recém-iniciado, o
  `CTRL_BREAK_EVENT` não gera ack da CLI em 5 s; o host é descartado. Na tela, a parada só veio no 3º clique, e os
  cliques 2 e 3 não aparecem no log. Reprodução: pedir "conte de 1 a 60 com `Start-Sleep 1`" e clicar em "Parar" logo
  que o comando começar.

## Processos órfãos encontrados

- `bash.exe` 10316 e 8256 (25/11, shell-snapshot do Claude), sem processo pai.
- Servidor de teste: PID 16952 na porta 4199. Parar com `taskkill /PID 16952 /T /F`.
