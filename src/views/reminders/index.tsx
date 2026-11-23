/**
 * Lembretes: a seção do painel Agentes (escrever "algo a fazer 18h, me lembre
 * 30 min antes") e o aviso que aparece no canto quando chega a hora.
 */
import { useEffect, useState } from "react";
import { Icon } from "@/shell/icons";
import { beforeLabel, parseReminder, whenLabel } from "./parse";
import { startReminders, useReminders } from "./store";
import "./reminders.css";

export { startReminders };

export function RemindersSection() {
  const list = useReminders((s) => s.list);
  const error = useReminders((s) => s.error);
  const [text, setText] = useState("");
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const t = setInterval(() => setNow(Date.now()), 30_000);
    return () => clearInterval(t);
  }, []);

  const preview = text.trim() ? parseReminder(text) : null;
  const upcoming = list.filter((r) => !r.fired || now < r.at);

  const add = () => {
    if (!preview) return;
    void useReminders.getState().add(preview);
    setText("");
  };

  return (
    <section className="ap-todos rm" aria-label="Lembretes">
      <header className="ap-head">
        <div className="ap-head-t">
          <h3>Lembretes</h3>
          <span className="ap-sub">valem para o Argus inteiro, não só esta sessão</span>
        </div>
      </header>
      <form
        className="ap-todo-add"
        onSubmit={(e) => {
          e.preventDefault();
          add();
        }}
      >
        <input
          value={text}
          placeholder="Ex.: reunião 18h, me lembre 30 min antes"
          aria-label="Novo lembrete"
          aria-describedby="rm-preview"
          maxLength={200}
          onChange={(e) => setText(e.target.value)}
        />
        <button type="submit" className="ap-ibtn" title="Criar lembrete" aria-label="Criar lembrete" disabled={!preview}>
          <Icon name="plus" size={14} />
        </button>
      </form>
      {text.trim() && (
        <p id="rm-preview" className={preview ? "ap-hint" : "ap-hint err"}>
          {preview
            ? `“${preview.text}” ${whenLabel(preview.at)} · aviso ${beforeLabel(preview.before)}`
            : "Diga a hora: 18h, 18:30, amanhã 9h…"}
        </p>
      )}
      {upcoming.length > 0 && (
        <ul className="ap-todo-list" role="list">
          {upcoming.map((r) => (
            <li key={r.id} className={r.fired ? "ap-todo done" : "ap-todo"}>
              <span className="rm-row">
                <Icon name="clock" size={13} />
                <span className="rm-body">
                  <span className="ap-todo-t">{r.text}</span>
                  <span className="rm-when">
                    {whenLabel(r.at)} · {r.fired ? "já avisado" : `aviso ${beforeLabel(r.before)}`}
                  </span>
                </span>
              </span>
              <button className="ap-ibtn" title="Apagar o lembrete" aria-label={`Apagar o lembrete ${r.text}`} onClick={() => void useReminders.getState().remove(r.id)}>
                <Icon name="close" size={12} />
              </button>
            </li>
          ))}
        </ul>
      )}
      {error && <p className="ap-hint err">{error}</p>}
    </section>
  );
}

/** Avisos na tela (canto inferior direito), até o usuário dispensar ou adiar. */
export function ReminderAlerts() {
  const alerts = useReminders((s) => s.alerts);
  if (alerts.length === 0) return null;
  return (
    <div className="rm-alerts" role="region" aria-label="Lembretes">
      {alerts.map((r) => {
        const left = Math.round((r.at - Date.now()) / 60_000);
        return (
          <div key={r.id} className="rm-alert" role="alert">
            <Icon name="clock" size={16} />
            <div className="rm-alert-body">
              <b>{r.text}</b>
              <span>
                {whenLabel(r.at)}
                {left > 0 ? ` · em ${beforeLabel(left).replace(" antes", "")}` : " · agora"}
              </span>
              <div className="rm-alert-actions">
                <button type="button" className="primary" onClick={() => useReminders.getState().dismiss(r.id)}>
                  Ok
                </button>
                <button type="button" onClick={() => void useReminders.getState().snooze(r.id, 10)}>
                  Lembrar em 10 min
                </button>
              </div>
            </div>
          </div>
        );
      })}
    </div>
  );
}
