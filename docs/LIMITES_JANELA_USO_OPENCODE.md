# Como identificar e calcular os limites de uso do OpenCode

_Anotação feita em 2026-11-24. Vale para o plugin `usage-window` do OpenCode nesta máquina e para a leitura que o Argus faz dele desde o mesmo dia. O Argus não lê nem guarda tokens de login das CLIs: só os contadores já calculados pelo plugin._

## Resposta curta

Eu não consulto a API do provider para obter a cota oficial. O limite é **estimado** a partir do consumo local. Quando o OpenCode reporta um erro de rate limit, o plugin registra quanto foi consumido até aquele ponto. A mediana desses registros é o limite usado para calcular o percentual.

## Onde está a fonte dos números

- Plugin: `C:\Users\kinga\.config\opencode\plugins\usage-window.ts`
- Estado salvo: `~/.local/share/opencode-usage-window/state.json`
- Variáveis de ambiente: `USAGE_WINDOW_FILE` troca o arquivo de estado; `USAGE_WINDOW_HOURS` troca a duração da janela (padrão: `5`)
- Ferramenta consultada: `usage_window`, com as ações `status`, `set_limit` e `reset_samples`

Cada `provider/modelo` tem seu próprio balde (`bucket`), por exemplo `opencode/mimo-v2.6-flash-free`.

## O que entra na conta

- A janela começa na **primeira resposta concluída** depois que a janela anterior expirou. Não é um limite de relógio fixo, como meia-noite.
- O fim da janela é `início + 5 horas`. É o horário mostrado como "reseta em".
- `tokens = input + output + reasoning`. O `cacheRead` é registrado à parte e **não** entra nesse total.
- Cada passo ou mensagem concluída conta como 1 requisição. Um id de evento evita contar o mesmo passo duas vezes.
- Modelos terminados em `-free`, `:free` ou `/free` têm custo `0`, na mesma regra do Argus (`src/api/util.ts`).

## Como o limite é identificado

O plugin detecta rate limit nestes eventos do OpenCode: `message.updated` com erro, `session.error`, `session.status` com `retry`, `session.step.failed`, `session.execution.failed` e `session.retry.scheduled`.

A detecção aceita `statusCode === 429` ou texto que combine `rate limit`, `too many requests`, `quota`, `429`, `resource exhausted` ou `usage limit` (`RATE_LIMIT_RE`).

Quando isso acontece numa janela ativa e ainda não tinha sido registrado:

1. `limitHitAt` guarda o horário do evento.
2. Uma amostra é salva com `tokens`, `requests`, `elapsedMs` e `at`.
3. Só **uma amostra por janela** entra na lista; no máximo 20 amostras são mantidas.

Sem amostra e sem limite manual, a saída mostra `?` e "% indisponível: ainda sem calibração". É o caso do `big-pickle` agora.

## Cálculo do percentual

```
tokens        = input + output + reasoning
est.tokens    = manualLimit.tokens    ?? mediana(samples.tokens)
est.requests  = manualLimit.requests  ?? mediana(samples.requests)

pct = arredondar( max( tokens / est.tokens , requests / est.requests ) * 100 )
```

O maior dos dois valores manda: se as requisições estourarem antes dos tokens, é elas que definem o percentual. A mediana é usada porque um único rate limit pode ter sido atingido num momento atípico; com várias janelas, o valor fica mais estável.

Apresentação:

- Barra de 20 blocos, cada bloco vale 5%.
- Avisos automáticos ao cruzar 50%, 75% e 90%.
- Projeção linear quando `0 < pct < 100` e a janela tem mais de 1 minuto: `ETA = decorrido / pct * (100 - pct)`. O aviso só aparece se o ETA for antes do reset; senão diz que a janela reseta antes.

## Leitura atual (snapshot de 2026-11-24)

| provider/modelo | tokens | reqs | amostras | limite estimado | uso |
|---|---:|---:|---:|---|---|
| `opencode/mimo-v2.6-flash-free` | 871.280 (733.105 in / 72.868 out / 65.307 reasoning) | 189 | 1 | 871.280 tokens / 189 reqs | 100%, rate limit às 09:46 |
| `opencode/big-pickle` | 471.859 (417.017 in / 22.430 out / 32.412 reasoning) | 104 | 0 | `? / ?` | sem calibração |

Os números mudam a cada requisição. Para conferir na hora, rode `usage_window` com `action=status` dentro do OpenCode.

## Ajustes manuais

- `set_limit` define tokens e/ou reqs manualmente. Substitui a mediana enquanto estiver definido.
- `reset_samples` apaga as amostras **e** o limite manual do balde indicado. Use quando o provider mudar de limites e a calibração antiga não fizer sentido.
- Os dois exigem `key` com o `provider/modelo` exato.

## Limitações para não esquecer

- É uma estimativa local, não a cota oficial do provider.
- Uma amostra por janela: com poucas janelas observadas, o limite pode estar impreciso.
- `cacheRead` fica fora do total de tokens; janelas com muito cache podem parecer mais baratas do que são.
- O arquivo de estado fica fora do repositório. Não coloque segredos nele nem nesta anotação.

## Leitura no Argus (desde 2026-11-24)

As janelas ativas aparecem no popover da bolinha de uso, ao lado das janelas informadas pela CLI. O módulo é `crates/core/src/usage_window.rs`; os comandos Tauri são `usage_windows`, `usage_window_set_limit` e `usage_window_reset_samples`.

- `USAGE_WINDOW_FILE` e `USAGE_WINDOW_HOURS` são respeitados, com os mesmos padrões do plugin.
- O arquivo é lido como JSON genérico e reescrito via temporário + `rename`, preservando campos que o Argus não conhece (`seen`, `diagnostics`).
- `set_limit` e `reset_samples` fazem o mesmo que a ferramenta do plugin. A limpeza pede confirmação na interface.
- Sem calibração o percentual aparece como "—". A lista é relida ao abrir o popover e a cada 30 segundos; janelas vencidas não entram.
- A fonte continua sendo a estimativa local descrita acima, não a cota oficial do provider.
