//! Tarefas agendadas: um pedido a um agente (ou só um lembrete) que dispara num
//! horário, uma vez ou com recorrência.
//!
//! Rodam só com o Argus aberto. Um laço confere a cada 30 s o que venceu e cria
//! uma tarefa normal pelo `Orchestrator` (o mesmo caminho de "Nova conversa"),
//! então histórico, uso e permissões valem igual. Se o Argus estava fechado no
//! horário, a primeira volta do laço roda UMA vez o que venceu (marcado como
//! atrasado) e calcula a próxima a partir de agora, sem acumular execuções.
//!
//! Horários no fuso do computador (`chrono::Local`). O cálculo é genérico no fuso
//! para os testes usarem um fixo.
use crate::agents::orchestrator::{Orchestrator, OrchestratorEvent, SubmitOptions, TaskId, TaskState, WorkMode};
use crate::db::now;
use chrono::{Datelike, Duration as CDuration, LocalResult, NaiveDate, TimeZone};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

/// Tolerância para considerar que o disparo foi no horário (e não atrasado).
const GRACE_SECS: i64 = 120;
/// Intervalo do laço.
const TICK: Duration = Duration::from_secs(30);

/// Quando o agendamento dispara.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum Recurrence {
    /// Uma vez, no instante (segundos desde 1970).
    Once { at: i64 },
    /// Todo dia às `HH:MM`.
    Daily { time: String },
    /// Nos dias da semana (0 = domingo … 6 = sábado) às `HH:MM`.
    Weekly { days: Vec<u8>, time: String },
    /// A cada N horas, contadas a partir da criação.
    EveryHours { hours: u32 },
    /// Expressão cron de 5 campos (minuto hora dia mês dia-da-semana).
    Cron { expr: String },
}

/// O que o agendamento faz.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ScheduleKind {
    /// Cria uma tarefa de agente com o pedido.
    #[default]
    Agent,
    /// Só mostra uma notificação com o texto.
    Reminder,
}

/// Resultado da última execução.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RunStatus {
    Running,
    Completed,
    Failed(String),
    Cancelled,
    /// Lembrete mostrado.
    Reminded,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Schedule {
    /// 0 = novo (ainda não gravado).
    #[serde(default)]
    pub id: i64,
    pub name: String,
    #[serde(default)]
    pub kind: ScheduleKind,
    #[serde(default)]
    pub project: Option<PathBuf>,
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub model: Option<String>,
    /// None = o padrão do projeto.
    #[serde(default)]
    pub mode: Option<WorkMode>,
    /// Pedido ao agente (ou texto do lembrete).
    pub prompt: String,
    pub recurrence: Recurrence,
    pub enabled: bool,
    #[serde(default)]
    pub created_at: i64,
    #[serde(default)]
    pub last_run: Option<i64>,
    #[serde(default)]
    pub next_run: Option<i64>,
    #[serde(default)]
    pub last_task: Option<TaskId>,
    #[serde(default)]
    pub last_status: Option<RunStatus>,
    /// A última execução foi uma recuperação (o Argus estava fechado no horário).
    #[serde(default)]
    pub last_late: bool,
}

/// Aviso do agendador para quem o hospeda (o desktop).
#[derive(Debug, Clone, PartialEq)]
pub enum SchedulerSignal {
    /// A lista ou o estado de algum agendamento mudou.
    Changed,
    /// Mostrar uma notificação do sistema.
    Notify { title: String, body: String, task: Option<TaskId> },
}

#[derive(Debug, thiserror::Error)]
pub enum ScheduleError {
    #[error("Sem banco de dados: as tarefas agendadas precisam do histórico ligado.")]
    NoDb,
    #[error("{0}")]
    Invalid(String),
    #[error("Agendamento não encontrado.")]
    NotFound,
    #[error("banco: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("{0}")]
    Json(#[from] serde_json::Error),
}

fn invalid<T>(msg: &str) -> Result<T, ScheduleError> {
    Err(ScheduleError::Invalid(msg.into()))
}

// ------------------------------------------------------------ cálculo de horário

/// `HH:MM` → (hora, minuto).
pub fn parse_time(s: &str) -> Option<(u32, u32)> {
    let (h, m) = s.trim().split_once(':')?;
    let (h, m) = (h.parse::<u32>().ok()?, m.parse::<u32>().ok()?);
    (h < 24 && m < 60).then_some((h, m))
}

/// Expressão cron simples já expandida.
#[derive(Debug, Clone, PartialEq)]
pub struct Cron {
    minutes: BTreeSet<u32>,
    hours: BTreeSet<u32>,
    days: BTreeSet<u32>,
    months: BTreeSet<u32>,
    weekdays: BTreeSet<u32>,
    days_any: bool,
    weekdays_any: bool,
}

fn cron_field(f: &str, min: u32, max: u32) -> Option<BTreeSet<u32>> {
    let mut out = BTreeSet::new();
    for part in f.split(',') {
        let (range, step) = match part.split_once('/') {
            Some((r, s)) => (r, s.parse::<u32>().ok().filter(|s| *s > 0)?),
            None => (part, 1),
        };
        let (a, b) = if range == "*" {
            (min, max)
        } else if let Some((a, b)) = range.split_once('-') {
            (a.parse().ok()?, b.parse().ok()?)
        } else {
            let v: u32 = range.parse().ok()?;
            // "5/15" = de 5 até o fim, de 15 em 15.
            (v, if part.contains('/') { max } else { v })
        };
        if a < min || b > max || a > b {
            return None;
        }
        out.extend((a..=b).step_by(step as usize));
    }
    (!out.is_empty()).then_some(out)
}

impl Cron {
    /// Cinco campos: minuto (0-59), hora (0-23), dia (1-31), mês (1-12) e dia da
    /// semana (0-7, 0 e 7 = domingo). Aceita `*`, números, `a-b`, listas e `/passo`.
    pub fn parse(expr: &str) -> Option<Cron> {
        let f: Vec<&str> = expr.split_whitespace().collect();
        if f.len() != 5 {
            return None;
        }
        let mut weekdays = cron_field(f[4], 0, 7)?;
        if weekdays.remove(&7) {
            weekdays.insert(0);
        }
        Some(Cron {
            minutes: cron_field(f[0], 0, 59)?,
            hours: cron_field(f[1], 0, 23)?,
            days: cron_field(f[2], 1, 31)?,
            months: cron_field(f[3], 1, 12)?,
            weekdays,
            days_any: f[2] == "*",
            weekdays_any: f[4] == "*",
        })
    }

    fn matches_date(&self, d: NaiveDate) -> bool {
        if !self.months.contains(&d.month()) {
            return false;
        }
        let dom = self.days.contains(&d.day());
        let dow = self.weekdays.contains(&d.weekday().num_days_from_sunday());
        // Como no cron: com os dois campos restritos, vale qualquer um deles.
        match (self.days_any, self.weekdays_any) {
            (true, true) => true,
            (true, false) => dow,
            (false, true) => dom,
            (false, false) => dom || dow,
        }
    }
}

/// Instante de `date` às `h:m` no fuso. Num buraco do horário de verão vale a
/// primeira hora existente depois; na hora repetida, a primeira ocorrência.
fn local_ts<Tz: TimeZone>(tz: &Tz, date: NaiveDate, h: u32, m: u32) -> Option<i64> {
    let naive = date.and_hms_opt(h, m, 0)?;
    for shift in 0..=2 {
        match tz.from_local_datetime(&(naive + CDuration::hours(shift))) {
            LocalResult::Single(t) => return Some(t.timestamp()),
            LocalResult::Ambiguous(a, _) => return Some(a.timestamp()),
            LocalResult::None => continue,
        }
    }
    None
}

fn local_date<Tz: TimeZone>(tz: &Tz, ts: i64) -> Option<NaiveDate> {
    Some(tz.timestamp_opt(ts, 0).single()?.naive_local().date())
}

/// Próximo disparo estritamente depois de `after`. `anchor` é a origem do "a cada
/// N horas" (a criação do agendamento). None = não há mais (uma vez, já passou).
pub fn next_after_tz<Tz: TimeZone>(rec: &Recurrence, after: i64, anchor: i64, tz: &Tz) -> Option<i64> {
    match rec {
        Recurrence::Once { at } => (*at > after).then_some(*at),
        Recurrence::Daily { time } => next_weekly(tz, after, &[0, 1, 2, 3, 4, 5, 6], time),
        Recurrence::Weekly { days, time } => next_weekly(tz, after, days, time),
        Recurrence::EveryHours { hours } => {
            let step = i64::from((*hours).max(1)) * 3600;
            if after < anchor {
                return Some(anchor + step);
            }
            Some(anchor + ((after - anchor) / step + 1) * step)
        }
        Recurrence::Cron { expr } => {
            let cron = Cron::parse(expr)?;
            let start = local_date(tz, after)?;
            // Quatro anos cobrem até "29 de fevereiro".
            for d in 0..(366 * 4 + 1) {
                let date = start + CDuration::days(d);
                if !cron.matches_date(date) {
                    continue;
                }
                for &h in &cron.hours {
                    for &m in &cron.minutes {
                        if let Some(ts) = local_ts(tz, date, h, m).filter(|ts| *ts > after) {
                            return Some(ts);
                        }
                    }
                }
            }
            None
        }
    }
}

fn next_weekly<Tz: TimeZone>(tz: &Tz, after: i64, days: &[u8], time: &str) -> Option<i64> {
    let (h, m) = parse_time(time)?;
    let start = local_date(tz, after)?;
    (0..=8).find_map(|d| {
        let date = start + CDuration::days(d);
        if !days.contains(&(date.weekday().num_days_from_sunday() as u8)) {
            return None;
        }
        local_ts(tz, date, h, m).filter(|ts| *ts > after)
    })
}

/// Como `next_after_tz`, no fuso do computador.
pub fn next_after(rec: &Recurrence, after: i64, anchor: i64) -> Option<i64> {
    next_after_tz(rec, after, anchor, &chrono::Local)
}

/// Confere os campos antes de gravar.
pub fn validate(s: &Schedule) -> Result<(), ScheduleError> {
    if s.name.trim().is_empty() {
        return invalid("Dê um nome à tarefa.");
    }
    if s.prompt.trim().is_empty() {
        return invalid(if s.kind == ScheduleKind::Agent { "Escreva o pedido ao agente." } else { "Escreva o texto do lembrete." });
    }
    if s.kind == ScheduleKind::Agent {
        if s.project.as_ref().is_none_or(|p| p.as_os_str().is_empty()) {
            return invalid("Escolha o projeto.");
        }
        if s.provider.trim().is_empty() {
            return invalid("Escolha o agente.");
        }
    }
    match &s.recurrence {
        Recurrence::Once { .. } => {}
        Recurrence::Daily { time } => {
            if parse_time(time).is_none() {
                return invalid("Horário inválido (use HH:MM).");
            }
        }
        Recurrence::Weekly { days, time } => {
            if parse_time(time).is_none() {
                return invalid("Horário inválido (use HH:MM).");
            }
            if days.is_empty() || days.iter().any(|d| *d > 6) {
                return invalid("Escolha pelo menos um dia da semana.");
            }
        }
        Recurrence::EveryHours { hours } => {
            if *hours == 0 || *hours > 24 * 7 {
                return invalid("O intervalo vai de 1 a 168 horas.");
            }
        }
        Recurrence::Cron { expr } => {
            if Cron::parse(expr).is_none() {
                return invalid("Expressão cron inválida: use 5 campos (minuto hora dia mês dia-da-semana).");
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- agendador

type SignalFn = Arc<dyn Fn(SchedulerSignal) + Send + Sync>;

struct Inner {
    orch: Orchestrator,
    /// Tarefa criada → agendamento que a criou (para avisar quando terminar).
    watching: Mutex<HashMap<TaskId, i64>>,
    signal: OnceLock<SignalFn>,
    /// Serializa leitura-alteração-gravação (laço e comandos).
    lock: Mutex<()>,
}

#[derive(Clone)]
pub struct Scheduler {
    inner: Arc<Inner>,
}

impl Scheduler {
    pub fn new(orch: Orchestrator) -> Self {
        Self {
            inner: Arc::new(Inner { orch, watching: Mutex::new(HashMap::new()), signal: OnceLock::new(), lock: Mutex::new(()) }),
        }
    }

    /// Liga o laço e o acompanhamento das tarefas criadas. Precisa do runtime tokio.
    /// A primeira volta roda logo: recupera o que venceu com o Argus fechado.
    pub fn start(&self, signal: impl Fn(SchedulerSignal) + Send + Sync + 'static) {
        if self.inner.signal.set(Arc::new(signal)).is_err() {
            return; // já ligado
        }
        let this = self.clone();
        let mut rx = self.inner.orch.subscribe();
        tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(OrchestratorEvent::StateChanged { task, state }) => this.on_task_state(task, &state),
                    Ok(_) => {}
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                    Err(_) => break,
                }
            }
        });
        let this = self.clone();
        tokio::spawn(async move {
            let mut iv = tokio::time::interval(TICK);
            loop {
                iv.tick().await;
                if let Err(e) = this.tick(now()) {
                    tracing::warn!("agendador: {e}");
                }
            }
        });
    }

    fn emit(&self, s: SchedulerSignal) {
        if let Some(f) = self.inner.signal.get() {
            f(s);
        }
    }

    fn db(&self) -> Result<&crate::db::Db, ScheduleError> {
        self.inner.orch.db().ok_or(ScheduleError::NoDb)
    }

    fn guard(&self) -> std::sync::MutexGuard<'_, ()> {
        self.inner.lock.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn list(&self) -> Result<Vec<Schedule>, ScheduleError> {
        let mut out = Vec::new();
        for (id, data) in self.db()?.list_schedules()? {
            match serde_json::from_str::<Schedule>(&data) {
                Ok(mut s) => {
                    s.id = id;
                    out.push(s);
                }
                Err(e) => tracing::warn!("agendamento {id} ilegível: {e}"),
            }
        }
        Ok(out)
    }

    fn get(&self, id: i64) -> Result<Schedule, ScheduleError> {
        self.list()?.into_iter().find(|s| s.id == id).ok_or(ScheduleError::NotFound)
    }

    fn store(&self, s: &Schedule) -> Result<(), ScheduleError> {
        if !self.db()?.update_schedule(s.id, &serde_json::to_string(s)?)? {
            return Err(ScheduleError::NotFound);
        }
        Ok(())
    }

    /// Cria (id 0) ou edita. Recalcula a próxima execução; preserva o histórico.
    pub fn save(&self, mut input: Schedule) -> Result<Schedule, ScheduleError> {
        input.name = input.name.trim().to_string();
        input.model = input.model.map(|m| m.trim().to_string()).filter(|m| !m.is_empty());
        validate(&input)?;
        let _g = self.guard();
        let t = now();
        if input.id != 0 {
            let old = self.get(input.id)?;
            input.created_at = old.created_at;
            input.last_run = old.last_run;
            input.last_task = old.last_task;
            input.last_status = old.last_status;
            input.last_late = old.last_late;
        } else {
            input.created_at = t;
            input.last_run = None;
            input.last_task = None;
            input.last_status = None;
            input.last_late = false;
        }
        input.next_run = if input.enabled { next_after(&input.recurrence, t, input.created_at) } else { None };
        if input.enabled && input.next_run.is_none() {
            return invalid("O horário já passou: escolha uma data futura.");
        }
        if input.id == 0 {
            input.id = self.db()?.insert_schedule(&serde_json::to_string(&input)?)?;
        } else {
            self.store(&input)?;
        }
        drop(_g);
        self.emit(SchedulerSignal::Changed);
        Ok(input)
    }

    pub fn set_enabled(&self, id: i64, enabled: bool) -> Result<Schedule, ScheduleError> {
        let mut s = self.get(id)?;
        s.enabled = enabled;
        self.save(s)
    }

    pub fn delete(&self, id: i64) -> Result<(), ScheduleError> {
        let _g = self.guard();
        if !self.db()?.delete_schedule(id)? {
            return Err(ScheduleError::NotFound);
        }
        self.inner.watching.lock().unwrap_or_else(|e| e.into_inner()).retain(|_, s| *s != id);
        drop(_g);
        self.emit(SchedulerSignal::Changed);
        Ok(())
    }

    /// Executa agora, sem mexer no horário da próxima.
    pub fn run_now(&self, id: i64) -> Result<Schedule, ScheduleError> {
        let _g = self.guard();
        let mut s = self.get(id)?;
        self.fire(&mut s, false, now());
        self.store(&s)?;
        drop(_g);
        self.emit(SchedulerSignal::Changed);
        Ok(s)
    }

    /// Uma volta do laço: dispara o que venceu até `t`. Devolve quantos dispararam.
    pub fn tick(&self, t: i64) -> Result<usize, ScheduleError> {
        let _g = self.guard();
        let mut fired = 0;
        for mut s in self.list()? {
            let Some(due) = s.next_run.filter(|n| s.enabled && *n <= t) else { continue };
            self.fire(&mut s, t - due > GRACE_SECS, t);
            // A próxima conta a partir de agora: execuções perdidas não se acumulam.
            s.next_run = next_after(&s.recurrence, t, s.created_at);
            if s.next_run.is_none() {
                s.enabled = false;
            }
            self.store(&s)?;
            fired += 1;
        }
        drop(_g);
        if fired > 0 {
            self.emit(SchedulerSignal::Changed);
        }
        Ok(fired)
    }

    fn fire(&self, s: &mut Schedule, late: bool, t: i64) {
        s.last_run = Some(t);
        s.last_late = late;
        let late_note = if late { " (atrasada: o Argus estava fechado no horário)" } else { "" };
        match s.kind {
            ScheduleKind::Reminder => {
                s.last_status = Some(RunStatus::Reminded);
                s.last_task = None;
                self.emit(SchedulerSignal::Notify { title: format!("Lembrete: {}{late_note}", s.name), body: s.prompt.clone(), task: None });
            }
            ScheduleKind::Agent => {
                let project = s.project.clone().unwrap_or_default();
                let opts = SubmitOptions { model: s.model.clone(), mode: s.mode, ..Default::default() };
                match self.inner.orch.submit_with(project, &s.provider, &s.prompt, opts) {
                    Ok(task) => {
                        s.last_task = Some(task);
                        s.last_status = Some(RunStatus::Running);
                        self.inner.watching.lock().unwrap_or_else(|e| e.into_inner()).insert(task, s.id);
                    }
                    Err(e) => {
                        let msg = e.to_string();
                        s.last_status = Some(RunStatus::Failed(msg.clone()));
                        self.emit(SchedulerSignal::Notify { title: format!("Tarefa agendada não iniciou: {}", s.name), body: msg, task: None });
                    }
                }
            }
        }
    }

    fn on_task_state(&self, task: TaskId, state: &TaskState) {
        let status = match state {
            TaskState::Queued | TaskState::Running => return,
            TaskState::Completed | TaskState::Applied | TaskState::Discarded => RunStatus::Completed,
            TaskState::Failed(e) => RunStatus::Failed(e.clone()),
            TaskState::Cancelled => RunStatus::Cancelled,
        };
        let Some(id) = self.inner.watching.lock().unwrap_or_else(|e| e.into_inner()).remove(&task) else { return };
        let _g = self.guard();
        let Ok(mut s) = self.get(id) else { return };
        if s.last_task == Some(task) {
            s.last_status = Some(status.clone());
            let _ = self.store(&s);
        }
        drop(_g);
        let (title, body) = match &status {
            RunStatus::Completed => (format!("Tarefa agendada concluída: {}", s.name), "Abra a conversa para ver o resultado.".to_string()),
            RunStatus::Failed(e) => (format!("Tarefa agendada falhou: {}", s.name), e.clone()),
            _ => (format!("Tarefa agendada cancelada: {}", s.name), String::new()),
        };
        self.emit(SchedulerSignal::Notify { title, body, task: Some(task) });
        self.emit(SchedulerSignal::Changed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;
    use chrono::{FixedOffset, Timelike};

    fn br() -> FixedOffset {
        FixedOffset::west_opt(3 * 3600).unwrap()
    }

    /// Instante em horário de Brasília.
    fn at(y: i32, mo: u32, d: u32, h: u32, mi: u32) -> i64 {
        br().with_ymd_and_hms(y, mo, d, h, mi, 0).unwrap().timestamp()
    }

    fn next(rec: &Recurrence, after: i64) -> Option<i64> {
        next_after_tz(rec, after, at(2026, 1, 1, 0, 0), &br())
    }

    #[test]
    fn parses_time() {
        assert_eq!(parse_time("08:05"), Some((8, 5)));
        assert_eq!(parse_time(" 23:59 "), Some((23, 59)));
        assert_eq!(parse_time("24:00"), None);
        assert_eq!(parse_time("8"), None);
    }

    #[test]
    fn daily_today_or_tomorrow() {
        let r = Recurrence::Daily { time: "09:00".into() };
        assert_eq!(next(&r, at(2026, 11, 19, 8, 0)), Some(at(2026, 11, 19, 9, 0)));
        assert_eq!(next(&r, at(2026, 11, 19, 9, 0)), Some(at(2026, 11, 20, 9, 0)));
        // Virada de mês e de ano.
        assert_eq!(next(&r, at(2026, 11, 30, 10, 0)), Some(at(2026, 12, 1, 9, 0)));
        assert_eq!(next(&r, at(2026, 12, 31, 10, 0)), Some(at(2027, 1, 1, 9, 0)));
    }

    #[test]
    fn weekly_picks_next_listed_day() {
        // 2026-11-19 é quinta (4). Segunda (1) e quarta (3).
        let r = Recurrence::Weekly { days: vec![1, 3], time: "18:30".into() };
        assert_eq!(next(&r, at(2026, 11, 19, 12, 0)), Some(at(2026, 11, 23, 18, 30)));
        assert_eq!(next(&r, at(2026, 11, 23, 18, 30)), Some(at(2026, 11, 25, 18, 30)));
        // Atravessa o fim do mês.
        assert_eq!(next(&r, at(2026, 11, 26, 0, 0)), Some(at(2026, 11, 30, 18, 30)));
    }

    #[test]
    fn every_hours_keeps_cadence() {
        let anchor = at(2026, 11, 19, 10, 0);
        let r = Recurrence::EveryHours { hours: 4 };
        assert_eq!(next_after_tz(&r, anchor, anchor, &br()), Some(at(2026, 11, 19, 14, 0)));
        assert_eq!(next_after_tz(&r, at(2026, 11, 19, 15, 0), anchor, &br()), Some(at(2026, 11, 19, 18, 0)));
        assert_eq!(next_after_tz(&r, at(2026, 11, 19, 22, 30), anchor, &br()), Some(at(2026, 11, 20, 2, 0)));
    }

    #[test]
    fn once_only_in_the_future() {
        let r = Recurrence::Once { at: at(2026, 12, 1, 8, 0) };
        assert_eq!(next(&r, at(2026, 11, 19, 8, 0)), Some(at(2026, 12, 1, 8, 0)));
        assert_eq!(next(&r, at(2026, 12, 1, 8, 0)), None);
    }

    #[test]
    fn cron_expressions() {
        let c = |e: &str, after| next(&Recurrence::Cron { expr: e.into() }, after);
        // Dias úteis às 9h, a partir de sexta à tarde → segunda.
        assert_eq!(c("0 9 * * 1-5", at(2026, 11, 20, 15, 0)), Some(at(2026, 11, 23, 9, 0)));
        // A cada 15 minutos.
        assert_eq!(c("*/15 * * * *", at(2026, 11, 19, 10, 7)), Some(at(2026, 11, 19, 10, 15)));
        // Dia 31: pula novembro (30 dias).
        assert_eq!(c("0 8 31 * *", at(2026, 11, 1, 0, 0)), Some(at(2026, 12, 31, 8, 0)));
        // 29 de fevereiro: só em 2028.
        assert_eq!(c("0 0 29 2 *", at(2026, 3, 1, 0, 0)), Some(at(2028, 2, 29, 0, 0)));
        // Domingo como 7.
        assert_eq!(c("30 7 * * 7", at(2026, 11, 19, 0, 0)), Some(at(2026, 11, 22, 7, 30)));
        assert!(Cron::parse("0 9 * *").is_none());
        assert!(Cron::parse("61 * * * *").is_none());
        assert!(Cron::parse("*/0 * * * *").is_none());
    }

    /// Fuso com horário de verão (FixedOffset não tem buraco): o buraco vira a hora seguinte.
    #[test]
    fn dst_gap_does_not_break() {
        #[derive(Clone, Copy, Debug)]
        struct Gap;
        // -03 até 2026-11-01 00:00 local, que pula para 01:00 (-02).
        impl TimeZone for Gap {
            type Offset = FixedOffset;
            fn from_offset(_: &FixedOffset) -> Self {
                Gap
            }
            fn offset_from_local_date(&self, d: &NaiveDate) -> LocalResult<FixedOffset> {
                self.offset_from_local_datetime(&d.and_hms_opt(12, 0, 0).unwrap())
            }
            fn offset_from_local_datetime(&self, l: &chrono::NaiveDateTime) -> LocalResult<FixedOffset> {
                let change = NaiveDate::from_ymd_opt(2026, 11, 1).unwrap().and_hms_opt(0, 0, 0).unwrap();
                if *l < change {
                    LocalResult::Single(FixedOffset::west_opt(3 * 3600).unwrap())
                } else if *l < change + CDuration::hours(1) {
                    LocalResult::None
                } else {
                    LocalResult::Single(FixedOffset::west_opt(2 * 3600).unwrap())
                }
            }
            fn offset_from_utc_date(&self, d: &NaiveDate) -> FixedOffset {
                self.offset_from_utc_datetime(&d.and_hms_opt(12, 0, 0).unwrap())
            }
            fn offset_from_utc_datetime(&self, u: &chrono::NaiveDateTime) -> FixedOffset {
                let change = NaiveDate::from_ymd_opt(2026, 11, 1).unwrap().and_hms_opt(3, 0, 0).unwrap();
                FixedOffset::west_opt(if *u < change { 3 } else { 2 } * 3600).unwrap()
            }
        }
        let r = Recurrence::Daily { time: "00:30".into() };
        let got = next_after_tz(&r, at(2026, 10, 31, 12, 0), 0, &Gap).unwrap();
        // 00:30 não existe no dia 1: vira 01:30 (-02) = 03:30 UTC.
        let want = FixedOffset::west_opt(2 * 3600).unwrap().with_ymd_and_hms(2026, 11, 1, 1, 30, 0).unwrap().timestamp();
        assert_eq!(got, want);
        assert_eq!(Gap.timestamp_opt(got, 0).unwrap().hour(), 1);
    }

    fn reminder(rec: Recurrence) -> Schedule {
        Schedule {
            id: 0,
            name: "Beber água".into(),
            kind: ScheduleKind::Reminder,
            project: None,
            provider: String::new(),
            model: None,
            mode: None,
            prompt: "Levanta e bebe água".into(),
            recurrence: rec,
            enabled: true,
            created_at: 0,
            last_run: None,
            next_run: None,
            last_task: None,
            last_status: None,
            last_late: false,
        }
    }

    #[test]
    fn validation_messages() {
        let mut s = reminder(Recurrence::Daily { time: "9:00".into() });
        assert!(validate(&s).is_ok());
        s.kind = ScheduleKind::Agent;
        assert!(validate(&s).unwrap_err().to_string().contains("projeto"));
        s.kind = ScheduleKind::Reminder;
        s.recurrence = Recurrence::Weekly { days: vec![], time: "09:00".into() };
        assert!(validate(&s).is_err());
        s.recurrence = Recurrence::Cron { expr: "x".into() };
        assert!(validate(&s).is_err());
    }

    #[tokio::test]
    async fn missed_run_fires_once_marked_late() {
        let orch = Orchestrator::with_db(vec![], 1, Some(Db::open_in_memory().unwrap()));
        let sched = Scheduler::new(orch);
        let got: Arc<Mutex<Vec<SchedulerSignal>>> = Arc::default();
        let sink = got.clone();
        // Sem `start` (não liga o laço): o sinal é registrado à mão.
        let _ = sched.inner.signal.set(Arc::new(move |s| sink.lock().unwrap().push(s)));
        let saved = sched.save(reminder(Recurrence::EveryHours { hours: 1 })).unwrap();
        let due = saved.next_run.unwrap();
        // O Argus ficou fechado por 5 horas: roda uma vez só.
        let fired = sched.tick(due + 5 * 3600).unwrap();
        assert_eq!(fired, 1);
        let s = sched.list().unwrap().remove(0);
        assert!(s.last_late);
        assert_eq!(s.last_status, Some(RunStatus::Reminded));
        assert!(s.next_run.unwrap() > due + 5 * 3600);
        assert_eq!(sched.tick(due + 5 * 3600).unwrap(), 0);
        let notes = got.lock().unwrap().iter().filter(|s| matches!(s, SchedulerSignal::Notify { .. })).count();
        assert_eq!(notes, 1);
        // Uma vez: desliga depois de rodar.
        let mut once = reminder(Recurrence::Once { at: now() + 60 });
        once.name = "Uma vez".into();
        let once = sched.save(once).unwrap();
        sched.tick(now() + 61).unwrap();
        let s = sched.list().unwrap().into_iter().find(|s| s.id == once.id).unwrap();
        assert!(!s.enabled && s.next_run.is_none() && !s.last_late);
        // Agente sem provedor: registra a falha em vez de quebrar.
        let mut ag = reminder(Recurrence::Daily { time: "09:00".into() });
        ag.kind = ScheduleKind::Agent;
        ag.project = Some(PathBuf::from("C:/nao-existe"));
        ag.provider = "nenhum".into();
        let ag = sched.save(ag).unwrap();
        let s = sched.run_now(ag.id).unwrap();
        assert!(matches!(s.last_status, Some(RunStatus::Failed(_))));
        sched.delete(ag.id).unwrap();
        assert!(sched.delete(ag.id).is_err());
    }
}
