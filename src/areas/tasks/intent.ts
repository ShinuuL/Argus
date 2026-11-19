/** Pedido de "nova tarefa" vindo de fora da área (paleta de comandos). */
const EVENT = "argus:nova-tarefa-agendada";
let pending = false;

/** Marca o pedido e avisa a área, se ela já estiver montada. */
export function requestNewTask(): void {
  pending = true;
  window.dispatchEvent(new Event(EVENT));
}

/** Consome o pedido pendente (true uma vez). */
export function takeNewTaskRequest(): boolean {
  const p = pending;
  pending = false;
  return p;
}

export function onNewTaskRequest(cb: () => void): () => void {
  window.addEventListener(EVENT, cb);
  return () => window.removeEventListener(EVENT, cb);
}
