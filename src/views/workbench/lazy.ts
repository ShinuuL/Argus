/** Carga preguiçosa do Monaco: o pacote só entra quando o primeiro arquivo é aberto. */
type MonacoModule = typeof import("./monaco");

let pending: Promise<MonacoModule> | null = null;

export function loadMonaco(): Promise<MonacoModule> {
  if (!pending) {
    pending = import("./monaco");
    // Falhou ao carregar: a próxima tentativa começa do zero.
    pending.catch(() => {
      pending = null;
    });
  }
  return pending;
}

/** Executa `fn` só se o Monaco já foi pedido (não força a carga). */
export function whenMonacoLoaded(fn: (m: MonacoModule) => void): void {
  if (pending) void pending.then(fn, () => undefined);
}
