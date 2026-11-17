import { Component, type ErrorInfo, type ReactNode } from "react";

interface State {
  error: Error | null;
  where?: string;
}

/**
 * Pega erros de render para a janela não ficar em branco: mostra a mensagem,
 * a pilha e um botão para recarregar a interface.
 */
export class ErrorBoundary extends Component<{ children: ReactNode }, State> {
  state: State = { error: null };

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error("Erro na interface:", error, info.componentStack);
    this.setState({ where: info.componentStack ?? undefined });
  }

  render() {
    const { error } = this.state;
    if (!error) return this.props.children;
    return (
      <div className="err-boundary" role="alert">
        <b>A interface encontrou um erro.</b>
        <pre>{`${error.message}\n\n${error.stack ?? ""}`}</pre>
        <button onClick={() => location.reload()}>Recarregar a interface</button>
      </div>
    );
  }
}
