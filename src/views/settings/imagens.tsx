/**
 * Fallback de imagem com a chave do Google AI Studio.
 *
 * A chave pode vir do ambiente ou ser colada aqui; nesse caso o Rust guarda só
 * na memória do processo e nunca a devolve. A geração acontece apenas quando o
 * usuário clica, para não transformar o plano free dos agentes em gasto surpresa.
 */
import { useCallback, useEffect, useState } from "react";
import { api, errorMessage, type ImagemArquivo, type ImagemStatus } from "@/api";
import { Lightbox } from "@/views/chat/Lightbox";

/** Conteúdo reaproveitado pela área de Configurações e pela view antiga. */
export function ImagensContent() {
  const [status, setStatus] = useState<ImagemStatus | null>(null);
  const [items, setItems] = useState<ImagemArquivo[]>([]);
  const [previews, setPreviews] = useState<Record<string, string>>({});
  const [prompt, setPrompt] = useState("");
  const [chave, setChave] = useState("");
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const [noticeTone, setNoticeTone] = useState<"info" | "err">("info");
  const [zoom, setZoom] = useState<{ src: string; name: string } | null>(null);

  const say = useCallback((text: string, tone: "info" | "err" = "info") => {
    setNotice(text);
    setNoticeTone(tone);
  }, []);

  const load = useCallback(async () => {
    try {
      const [s, list] = await Promise.all([api.imagemStatus(), api.imagemListar()]);
      setStatus(s);
      setItems(list);
    } catch (e) {
      say(errorMessage(e), "err");
    }
  }, [say]);

  useEffect(() => {
    void load();
  }, [load]);

  useEffect(() => {
    let alive = true;
    for (const item of items) {
      if (previews[item.path]) continue;
      void api
        .imagemLer(item.path)
        .then((d) => {
          if (alive) setPreviews((p) => ({ ...p, [item.path]: `data:${d.mime};base64,${d.data}` }));
        })
        .catch(() => undefined);
    }
    return () => {
      alive = false;
    };
  }, [items, previews]);

  const salvarChave = async () => {
    if (!chave.trim()) return;
    const valor = chave;
    try {
      await api.imagemChaveDefinir(valor);
      setChave("");
      say("Chave guardada só nesta sessão. Ela tem precedência sobre a variável de ambiente.");
      await load();
    } catch (e) {
      say(errorMessage(e), "err");
    }
  };

  const limparChave = async () => {
    try {
      await api.imagemChaveLimpar();
      say("Chave da tela removida.");
      await load();
    } catch (e) {
      say(errorMessage(e), "err");
    }
  };

  const generate = async () => {
    if (!prompt.trim() || busy) return;
    setBusy(true);
    say("");
    try {
      const img = await api.imagemGerar(prompt);
      setPrompt("");
      say(`Imagem salva: ${img.name}${img.text ? ` — ${img.text}` : ""}`);
      await load();
    } catch (e) {
      say(errorMessage(e), "err");
    } finally {
      setBusy(false);
    }
  };

  return (
    <>
      <div className="set-card">
        <b>Origem da chave</b>
        <p className="small muted">
          {status?.configured
            ? status.from_ui
              ? "Configurada nesta sessão (tela)."
              : `Configurada em ${status.source}.`
            : "Não configurada."}{" "}
          Modelo: {status?.model ?? "…"}.
        </p>
        <p className="small muted">{status?.hint ?? "Consultando o ambiente…"}</p>
        <p className="small">
          Aviso: os modelos de imagem do Gemini não têm free tier. Cada geração usa a chave configurada e só acontece
          quando você pede.
        </p>
        <label className="set-field">
          <span className="small">Chave do Google AI Studio</span>
          <input
            type="password"
            value={chave}
            autoComplete="off"
            placeholder={status?.from_ui ? "Chave da tela ativa — cole outra para trocar" : "Cole a chave para usar nesta sessão"}
            onChange={(e) => setChave(e.target.value)}
          />
        </label>
        <div className="set-img-actions">
          <button type="button" disabled={!chave.trim()} onClick={() => void salvarChave()}>
            Guardar chave
          </button>
          {status?.from_ui && (
            <button type="button" onClick={() => void limparChave()}>
              Limpar chave da tela
            </button>
          )}
        </div>
        <p className="small muted">
          Guardada assim, a chave vale só até reiniciar o Argus. Ela tem precedência sobre a variável de ambiente e nunca é
          gravada em disco, logada ou devolvida à interface.
        </p>
        <label className="set-field">
          <span className="small">Pedido da imagem</span>
          <textarea
            rows={3}
            value={prompt}
            placeholder="Exemplo: um gato astronauta em estilo de gravura"
            onChange={(e) => setPrompt(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) void generate();
            }}
          />
        </label>
        <button type="button" className="primary" disabled={busy || !prompt.trim()} onClick={() => void generate()}>
          {busy ? "Gerando…" : "Gerar imagem"}
        </button>
        {notice && <p className={noticeTone === "err" ? "small err" : "small"}>{notice}</p>}
        <p className="small muted">
          No chat, o comando <code>/imagem &lt;pedido&gt;</code> gera a imagem e já a deixa anexada à caixa de mensagem.
        </p>
      </div>

      <div className="set-card">
        <b>Galeria local</b>
        {items.length === 0 ? (
          <p className="small muted">Nenhuma imagem gerada ainda.</p>
        ) : (
          <div className="set-img-grid">
            {items.map((item) => (
              <button
                key={item.path}
                type="button"
                className="set-img-card"
                title={`${item.name} — abrir em tamanho real`}
                onClick={() => {
                  const src = previews[item.path];
                  if (src) setZoom({ src, name: item.name });
                }}
              >
                {previews[item.path] ? (
                  <img src={previews[item.path]} alt={item.name} />
                ) : (
                  <span className="small muted">Carregando…</span>
                )}
                <span className="small muted">{item.name}</span>
              </button>
            ))}
          </div>
        )}
      </div>
      {zoom && <Lightbox src={zoom.src} alt={zoom.name} onClose={() => setZoom(null)} />}
    </>
  );
}

/** View antiga do workbench (ainda usada por abas restauradas). */
export default function ImagensPanel() {
  return (
    <section className="set-section">
      <h3>Imagens</h3>
      <p className="page-lead">
        Fallback pago para quando o plano free do agente não cobre imagem. A chave do Google AI Studio vem do ambiente ou
        da tela (só nesta sessão); os agentes continuam usando o login das CLIs oficiais.
      </p>
      <ImagensContent />
    </section>
  );
}
