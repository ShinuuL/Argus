//! Área de trabalho estilo VS Code: activity bar, barra lateral, grupos de editores
//! com split (até 3), abas de vários tipos e status bar.
//!
//! Uso pelo integrador:
//! ```ignore
//! let wb = provide_workbench();               // no App, uma vez
//! Workbench { side: rsx!{ ... conforme wb.activity() ... }, status_left: rsx!{}, status_right: rsx!{} }
//! use_workbench().open_session(task_id);      // de qualquer componente
//! ```
//! O workbench também observa `AppState::open_file` e abre o arquivo como aba de pré-visualização.
//!
//! Editor: cada grupo tem uma instância Monaco; os modelos (um por arquivo) ficam num
//! registro global no JS (`window.__hxModels`), então o mesmo arquivo em dois grupos
//! compartilha o conteúdo e o estado de "modificado".

use super::dashboard::Dashboard;
use super::editor::{assets_base_url, language_for, unique_dom_id};
use super::metrics::MetricsView;
use super::terminal::Terminal;
use crate::state::{use_app, use_orchestrator};
use dioxus::prelude::*;
use argus_core::agents::orchestrator::{OrchestratorEvent, TaskId, TaskState};
use argus_core::agents::provider::AgentEvent;
use argus_core::pty::CommandSpec;
use serde_json::{json, Value};
use std::cell::RefCell;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::rc::Rc;

pub const MAX_GROUPS: usize = 3;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Activity {
    Explorer,
    Agents,
    Deps,
    Metrics,
    Remote,
}

impl Activity {
    pub const ALL: [Activity; 5] = [Activity::Explorer, Activity::Agents, Activity::Deps, Activity::Metrics, Activity::Remote];
    pub fn label(self) -> &'static str {
        match self {
            Activity::Explorer => "Explorador",
            Activity::Agents => "Agentes",
            Activity::Deps => "Dependências",
            Activity::Metrics => "Métricas",
            Activity::Remote => "Remoto",
        }
    }
    fn icon(self) -> &'static str {
        // Caminhos SVG (viewBox 0 0 24 24, traço).
        match self {
            Activity::Explorer => "M4 4h7l2 2h7v14H4z",
            Activity::Agents => "M12 3a4 4 0 110 8 4 4 0 010-8zM4 21c0-4 4-7 8-7s8 3 8 7",
            Activity::Deps => "M12 3l8 4.5v9L12 21l-8-4.5v-9zM12 12l8-4.5M12 12v9M12 12L4 7.5",
            Activity::Metrics => "M4 20V10M10 20V4M16 20v-7M22 20H2",
            Activity::Remote => "M7 2h10v20H7zM11 18h2",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SplitDir {
    /// Grupos lado a lado.
    Horizontal,
    /// Grupos empilhados.
    Vertical,
}

#[derive(Clone, PartialEq, Debug)]
pub enum TabKind {
    File(PathBuf),
    Session(TaskId),
    Terminal { title: String, cwd: PathBuf, command: Option<CommandSpec> },
    Dashboard,
    Metrics,
}

impl TabKind {
    /// Mesma "identidade" (para reaproveitar aba já aberta). Terminais nunca coincidem.
    fn same(&self, o: &TabKind) -> bool {
        match (self, o) {
            (TabKind::File(a), TabKind::File(b)) => a == b,
            (TabKind::Session(a), TabKind::Session(b)) => a == b,
            (TabKind::Dashboard, TabKind::Dashboard) | (TabKind::Metrics, TabKind::Metrics) => true,
            _ => false,
        }
    }
    pub fn title(&self) -> String {
        match self {
            TabKind::File(p) => p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
            TabKind::Session(id) => format!("Sessão #{id}"),
            TabKind::Terminal { title, .. } => title.clone(),
            TabKind::Dashboard => "Painel".into(),
            TabKind::Metrics => "Métricas".into(),
        }
    }
    fn tooltip(&self) -> String {
        match self {
            TabKind::File(p) => p.display().to_string(),
            TabKind::Terminal { cwd, .. } => format!("Terminal em {}", cwd.display()),
            k => k.title(),
        }
    }
    /// (texto do ícone, cor).
    fn icon(&self) -> (&'static str, &'static str) {
        match self {
            TabKind::File(p) => file_icon(p),
            TabKind::Session(_) => ("AI", "#b48ef5"),
            TabKind::Terminal { .. } => (">_", "#5fd38d"),
            TabKind::Dashboard => ("\u{25A6}", "#7aa2f7"),
            TabKind::Metrics => ("$", "#e8b04a"),
        }
    }
}

fn file_icon(p: &Path) -> (&'static str, &'static str) {
    match language_for(p) {
        "rust" => ("RS", "#dea584"),
        "javascript" => ("JS", "#f1e05a"),
        "typescript" => ("TS", "#3178c6"),
        "json" => ("{}", "#cbcb41"),
        "ini" => ("\u{2699}", "#9aa0aa"),
        "python" => ("PY", "#3572a5"),
        "markdown" => ("M\u{2193}", "#7aa2f7"),
        "html" => ("<>", "#e34c26"),
        "css" | "scss" => ("#", "#563d7c"),
        "java" | "kotlin" => ("J", "#b07219"),
        "csharp" => ("C#", "#178600"),
        "sql" => ("DB", "#e38c00"),
        "shell" | "powershell" => ("$", "#89e051"),
        "yaml" | "xml" => ("\u{2261}", "#cb171e"),
        _ => ("\u{25A1}", "#9aa0aa"),
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct WbTab {
    pub id: u64,
    pub kind: TabKind,
    /// Aba de pré-visualização (itálico): é substituída pelo próximo arquivo aberto
    /// até ser editada, receber duplo clique ou ser fixada.
    pub preview: bool,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Group {
    pub id: u64,
    pub tabs: Vec<WbTab>,
    pub active: Option<u64>,
}

impl Group {
    fn active_tab(&self) -> Option<&WbTab> {
        self.active.and_then(|a| self.tabs.iter().find(|t| t.id == a))
    }
}

/// Estado do workbench (Copy; todos os campos são sinais).
#[derive(Clone, Copy)]
pub struct WorkbenchState {
    pub groups: Signal<Vec<Group>>,
    pub active_group: Signal<u64>,
    pub split: Signal<SplitDir>,
    pub activity: Signal<Activity>,
    pub sidebar_visible: Signal<bool>,
    /// Arquivos com alterações não salvas.
    pub dirty: Signal<HashSet<PathBuf>>,
    drag: Signal<Option<u64>>,
    next_id: Signal<u64>,
}

/// Cria o estado e o coloca no contexto. Chame uma vez no `App`.
pub fn provide_workbench() -> WorkbenchState {
    use_context_provider(|| WorkbenchState {
        groups: Signal::new(vec![Group { id: 0, tabs: vec![], active: None }]),
        active_group: Signal::new(0),
        split: Signal::new(SplitDir::Horizontal),
        activity: Signal::new(Activity::Explorer),
        sidebar_visible: Signal::new(true),
        dirty: Signal::new(HashSet::new()),
        drag: Signal::new(None),
        next_id: Signal::new(1),
    })
}

pub fn use_workbench() -> WorkbenchState {
    use_context::<WorkbenchState>()
}

impl WorkbenchState {
    fn fresh_id(mut self) -> u64 {
        let id = *self.next_id.peek();
        self.next_id.set(id + 1);
        id
    }

    fn group_index(&self, gid: u64) -> Option<usize> {
        self.groups.peek().iter().position(|g| g.id == gid)
    }

    fn active_index(&self) -> usize {
        self.group_index(*self.active_group.peek()).unwrap_or(0)
    }

    /// Abre (ou foca) uma aba no grupo ativo.
    pub fn open(mut self, kind: TabKind, preview: bool) {
        let gi = self.active_index();
        let new_id = self.fresh_id();
        let mut groups = self.groups.write();
        let Some(g) = groups.get_mut(gi) else { return };
        if let Some(t) = g.tabs.iter_mut().find(|t| t.kind.same(&kind)) {
            if !preview {
                t.preview = false;
            }
            g.active = Some(t.id);
            return;
        }
        let tab = WbTab { id: new_id, kind, preview };
        if preview {
            if let Some(t) = g.tabs.iter_mut().find(|t| t.preview) {
                t.kind = tab.kind;
                g.active = Some(t.id);
                return;
            }
        }
        let pos = g
            .active
            .and_then(|a| g.tabs.iter().position(|t| t.id == a))
            .map(|i| i + 1)
            .unwrap_or(g.tabs.len());
        g.tabs.insert(pos, tab);
        g.active = Some(new_id);
    }

    pub fn open_file(self, path: PathBuf, preview: bool) {
        self.open(TabKind::File(path), preview);
    }
    pub fn open_session(self, task: TaskId) {
        self.open(TabKind::Session(task), false);
    }
    pub fn open_terminal(self, title: String, cwd: PathBuf, command: Option<CommandSpec>) {
        self.open(TabKind::Terminal { title, cwd, command }, false);
    }
    pub fn open_dashboard(self) {
        self.open(TabKind::Dashboard, false);
    }
    pub fn open_metrics(self) {
        self.open(TabKind::Metrics, false);
    }

    pub fn activate(mut self, gid: u64, tab: u64) {
        if let Some(gi) = self.group_index(gid) {
            self.groups.write()[gi].active = Some(tab);
        }
        self.active_group.set(gid);
    }

    /// Fixa a aba (deixa de ser pré-visualização).
    pub fn pin(mut self, tab: u64) {
        for g in self.groups.write().iter_mut() {
            for t in g.tabs.iter_mut().filter(|t| t.id == tab) {
                t.preview = false;
            }
        }
    }

    fn pin_file(mut self, path: &Path) {
        let has = self.groups.peek().iter().any(|g| g.tabs.iter().any(|t| t.preview && t.kind == TabKind::File(path.to_path_buf())));
        if has {
            for g in self.groups.write().iter_mut() {
                for t in g.tabs.iter_mut() {
                    if matches!(&t.kind, TabKind::File(p) if p == path) {
                        t.preview = false;
                    }
                }
            }
        }
    }

    fn is_open_elsewhere(&self, path: &Path, except_tab: u64) -> bool {
        self.groups
            .peek()
            .iter()
            .any(|g| g.tabs.iter().any(|t| t.id != except_tab && matches!(&t.kind, TabKind::File(p) if p == path)))
    }

    /// Fecha uma aba. Pede confirmação se for o último editor de um arquivo modificado.
    /// Retorna false se o usuário cancelou.
    pub fn close_tab(mut self, gid: u64, tab: u64) -> bool {
        let Some(gi) = self.group_index(gid) else { return true };
        let kind = self.groups.peek()[gi].tabs.iter().find(|t| t.id == tab).map(|t| t.kind.clone());
        if let Some(TabKind::File(p)) = &kind {
            if !self.is_open_elsewhere(p, tab) && self.dirty.peek().contains(p) {
                let discard = rfd::MessageDialog::new()
                    .set_title("Alterações não salvas")
                    .set_description(format!("{} tem alterações não salvas. Fechar e descartar?", p.display()))
                    .set_buttons(rfd::MessageButtons::YesNo)
                    .show();
                if discard != rfd::MessageDialogResult::Yes {
                    return false;
                }
                self.dirty.write().remove(p);
            }
        }
        let mut groups = self.groups.write();
        let g = &mut groups[gi];
        let Some(pos) = g.tabs.iter().position(|t| t.id == tab) else { return true };
        g.tabs.remove(pos);
        if g.active == Some(tab) {
            g.active = g.tabs.get(pos.min(g.tabs.len().saturating_sub(1))).map(|t| t.id);
        }
        // Grupo vazio some (se não for o único).
        if g.tabs.is_empty() && groups.len() > 1 {
            groups.remove(gi);
            let next = groups[gi.min(groups.len() - 1)].id;
            drop(groups);
            if *self.active_group.peek() == gid {
                self.active_group.set(next);
            }
        }
        true
    }

    pub fn close_others(self, gid: u64, keep: u64) {
        let Some(gi) = self.group_index(gid) else { return };
        let ids: Vec<u64> = self.groups.peek()[gi].tabs.iter().filter(|t| t.id != keep).map(|t| t.id).collect();
        for id in ids {
            self.close_tab(gid, id);
        }
    }

    pub fn close_active(self) {
        let gid = *self.active_group.peek();
        let tab = self.group_index(gid).and_then(|gi| self.groups.peek()[gi].active);
        if let Some(t) = tab {
            self.close_tab(gid, t);
        }
    }

    /// Próxima/anterior aba do grupo ativo (Ctrl+Tab / Ctrl+Shift+Tab).
    pub fn cycle(mut self, back: bool) {
        let gi = self.active_index();
        if let Some(g) = self.groups.write().get_mut(gi) {
            cycle_in(g, back);
        }
    }

    /// Divide o editor (Ctrl+\): novo grupo à direita/abaixo com uma cópia da aba ativa
    /// (terminais não são duplicados). Limite de `MAX_GROUPS`.
    pub fn split_active(mut self) {
        if self.groups.peek().len() >= MAX_GROUPS {
            return;
        }
        let gi = self.active_index();
        let gid = self.fresh_id();
        let tid = self.fresh_id();
        let copy = self.groups.peek().get(gi).and_then(|g| g.active_tab().cloned());
        let tabs = match copy {
            Some(t) if !matches!(t.kind, TabKind::Terminal { .. }) => vec![WbTab { id: tid, kind: t.kind, preview: false }],
            _ => vec![],
        };
        let active = tabs.first().map(|t| t.id);
        self.groups.write().insert(gi + 1, Group { id: gid, tabs, active });
        self.active_group.set(gid);
    }

    /// Move uma aba para outro grupo (antes de `before`, ou no fim).
    pub fn move_tab(mut self, tab: u64, to_gid: u64, before: Option<u64>) {
        let moved = move_tab_in(&mut self.groups.write(), tab, to_gid, before);
        if moved {
            self.active_group.set(to_gid);
        }
    }

    pub fn toggle_split_dir(mut self) {
        let d = *self.split.peek();
        self.split.set(if d == SplitDir::Horizontal { SplitDir::Vertical } else { SplitDir::Horizontal });
    }

    /// Clique na activity bar: troca de vista, ou recolhe a barra lateral se já ativa.
    pub fn select_activity(mut self, a: Activity) {
        if *self.activity.peek() == a {
            let v = *self.sidebar_visible.peek();
            self.sidebar_visible.set(!v);
        } else {
            self.activity.set(a);
            self.sidebar_visible.set(true);
        }
    }
}

/// Ativa a próxima/anterior aba de `g` (circular).
fn cycle_in(g: &mut Group, back: bool) {
    let n = g.tabs.len();
    if n == 0 {
        return;
    }
    let cur = g.active.and_then(|a| g.tabs.iter().position(|t| t.id == a)).unwrap_or(0);
    let next = if back { (cur + n - 1) % n } else { (cur + 1) % n };
    g.active = Some(g.tabs[next].id);
}

/// Move a aba `tab` para o grupo `to_gid` (antes de `before`, ou no fim).
/// Se o destino já tem a mesma aba (ex.: mesmo arquivo), só foca lá e fecha a origem.
/// Grupo de origem vazio é removido (se não for o único). Retorna se algo mudou.
fn move_tab_in(groups: &mut Vec<Group>, tab: u64, to_gid: u64, before: Option<u64>) -> bool {
    if before == Some(tab) {
        return false;
    }
    let Some((from_gi, pos)) = groups
        .iter()
        .enumerate()
        .find_map(|(gi, g)| g.tabs.iter().position(|t| t.id == tab).map(|p| (gi, p)))
    else {
        return false;
    };
    let Some(to_gi) = groups.iter().position(|g| g.id == to_gid) else { return false };
    let t = groups[from_gi].tabs.remove(pos);
    let from = &mut groups[from_gi];
    if from.active == Some(tab) {
        from.active = from.tabs.get(pos.min(from.tabs.len().saturating_sub(1))).map(|t| t.id);
    }
    let dest = &mut groups[to_gi];
    if let Some(existing) = dest.tabs.iter().find(|x| x.kind.same(&t.kind)).map(|x| x.id) {
        dest.active = Some(existing);
    } else {
        let at = before.and_then(|b| dest.tabs.iter().position(|x| x.id == b)).unwrap_or(dest.tabs.len());
        dest.active = Some(t.id);
        dest.tabs.insert(at, t);
    }
    if groups[from_gi].tabs.is_empty() && groups.len() > 1 {
        groups.remove(from_gi);
    }
    true
}

const KEYS_JS: &str = r#"
if (window.__hxKeys) document.removeEventListener('keydown', window.__hxKeys, true);
window.__hxKeys = (e) => {
  if (!e.ctrlKey || e.altKey) return;
  let k = null;
  if (e.key === 'Tab') k = e.shiftKey ? 'prev' : 'next';
  else if (e.key === 'w' || e.key === 'W') k = 'close';
  else if (e.key === '\\' || e.code === 'Backslash' || e.code === 'IntlBackslash') k = 'split';
  if (!k) return;
  e.preventDefault(); e.stopPropagation();
  dioxus.send(k);
};
document.addEventListener('keydown', window.__hxKeys, true);
await new Promise(() => {});
"#;

/// Workbench completo. `side` é o conteúdo da barra lateral (escolha pelo `activity`);
/// `status_left`/`status_right` vão na status bar (player, pet etc.).
#[component]
pub fn Workbench(side: Element, bottom: Element, status_left: Element, status_right: Element) -> Element {
    let wb = use_workbench();
    let app = use_app();

    // Atalhos globais (capturados antes do Monaco/xterm).
    use_future(move || async move {
        let mut ev = document::eval(KEYS_JS);
        while let Ok(k) = ev.recv::<String>().await {
            match k.as_str() {
                "next" => wb.cycle(false),
                "prev" => wb.cycle(true),
                "close" => {
                    wb.close_active();
                }
                "split" => wb.split_active(),
                _ => {}
            }
        }
    });

    // Arquivo escolhido na árvore -> aba de pré-visualização.
    use_effect(move || {
        if let Some(p) = (app.open_file)() {
            wb.open_file(p, true);
        }
    });

    let groups = (wb.groups)();
    let active_gid = (wb.active_group)();
    let split = (wb.split)();
    let activity = (wb.activity)();
    let n_groups = groups.len();

    rsx! {
        style { {WB_CSS} }
        div { class: "wb",
            div { class: "wb-main",
                nav { class: "wb-activity",
                    for a in Activity::ALL {
                        button {
                            key: "{a.label()}",
                            class: if activity == a && (wb.sidebar_visible)() { "wb-act active" } else { "wb-act" },
                            title: "{a.label()}",
                            onclick: move |_| wb.select_activity(a),
                            svg { view_box: "0 0 24 24", width: "22", height: "22", fill: "none", stroke: "currentColor", stroke_width: "1.6", stroke_linejoin: "round", stroke_linecap: "round",
                                path { d: "{a.icon()}" }
                            }
                        }
                    }
                }
                if (wb.sidebar_visible)() {
                    aside { class: "wb-side",
                        div { class: "wb-side-title", "{activity.label()}" }
                        div { class: "wb-side-body", {side} }
                    }
                }
                div { class: "wb-center",
                    div { class: if split == SplitDir::Horizontal { "wb-groups wb-row" } else { "wb-groups wb-col" },
                        for g in groups {
                            EditorGroup { key: "{g.id}", group: g.clone(), focused: g.id == active_gid, can_split: n_groups < MAX_GROUPS }
                        }
                    }
                    {bottom}
                }
                super::servers::ServersPanel {}
            }
            footer { class: "wb-status",
                div { class: "wb-status-l", {status_left} }
                div { class: "wb-status-r",
                    {status_right}
                    super::servers::ServersStatusButton {}
                    button {
                        class: "wb-sbtn",
                        title: "Alternar divisão horizontal/vertical",
                        onclick: move |_| wb.toggle_split_dir(),
                        if split == SplitDir::Horizontal { "\u{2502} lado a lado" } else { "\u{2500} empilhado" }
                    }
                }
            }
        }
    }
}

#[component]
fn EditorGroup(group: Group, focused: bool, can_split: bool) -> Element {
    let wb = use_workbench();
    let dirty = (wb.dirty)();
    let gid = group.id;
    let active = group.active_tab().cloned();
    let show_editor = matches!(active.as_ref().map(|t| &t.kind), Some(TabKind::File(_)));

    rsx! {
        div {
            class: if focused { "wb-group focused" } else { "wb-group" },
            onmousedown: move |_| { if *wb.active_group.peek() != gid { wb.clone().active_group.set(gid) } },
            div {
                class: "wb-tabs",
                ondragover: move |e: DragEvent| e.prevent_default(),
                ondrop: move |e: DragEvent| {
                    e.prevent_default();
                    let mut drag = wb.drag;
                    if let Some(t) = drag.take() { wb.move_tab(t, gid, None) }
                },
                for t in group.tabs.iter().cloned() {
                    {
                        let is_active = group.active == Some(t.id);
                        let modified = matches!(&t.kind, TabKind::File(p) if dirty.contains(p));
                        let (ic, color) = t.kind.icon();
                        let title = t.kind.title();
                        let tip = t.kind.tooltip();
                        let tid = t.id;
                        let mut cls = String::from("wb-tab");
                        if is_active { cls.push_str(" active"); }
                        if t.preview { cls.push_str(" preview"); }
                        if modified { cls.push_str(" dirty"); }
                        rsx! {
                            div {
                                key: "{tid}",
                                class: "{cls}",
                                title: "{tip}",
                                draggable: "true",
                                ondragstart: move |_| { let mut d = wb.drag; d.set(Some(tid)); },
                                ondragend: move |_| { let mut d = wb.drag; d.set(None); },
                                ondragover: move |e: DragEvent| e.prevent_default(),
                                ondrop: move |e: DragEvent| {
                                    e.prevent_default();
                                    e.stop_propagation();
                                    let mut drag = wb.drag;
                                    if let Some(t) = drag.take() { wb.move_tab(t, gid, Some(tid)) }
                                },
                                onclick: move |_| wb.activate(gid, tid),
                                ondoubleclick: move |_| wb.pin(tid),
                                onmouseup: move |e: MouseEvent| {
                                    if e.trigger_button() == Some(dioxus::html::input_data::MouseButton::Auxiliary) {
                                        wb.close_tab(gid, tid);
                                    }
                                },
                                span { class: "wb-ic", style: "color:{color}", "{ic}" }
                                span { class: "wb-title", "{title}" }
                                span {
                                    class: "wb-x",
                                    title: if modified { "Modificado (fechar)" } else { "Fechar (Ctrl+W)" },
                                    onclick: move |e| { e.stop_propagation(); wb.close_tab(gid, tid); },
                                    span { class: "wb-dot", "\u{25CF}" }
                                    span { class: "wb-cross", "\u{00D7}" }
                                }
                            }
                        }
                    }
                }
                div { class: "wb-tabs-fill" }
                div { class: "wb-tab-actions",
                    if can_split {
                        button { class: "wb-sbtn", title: "Dividir editor (Ctrl+\\)", onclick: move |_| { wb.clone().active_group.set(gid); wb.split_active(); }, "\u{2AFF}" }
                    }
                    if let Some(a) = active.as_ref() {
                        {
                            let keep = a.id;
                            rsx! { button { class: "wb-sbtn", title: "Fechar outras", onclick: move |_| wb.close_others(gid, keep), "\u{22EF}" } }
                        }
                    }
                }
            }
            div {
                class: "wb-body",
                ondragover: move |e: DragEvent| e.prevent_default(),
                ondrop: move |e: DragEvent| {
                    e.prevent_default();
                    let mut drag = wb.drag;
                    if let Some(t) = drag.take() { wb.move_tab(t, gid, None) }
                },
                div { class: "pane", style: if show_editor { "display:block" } else { "display:none" },
                    GroupEditor { gid }
                }
                // Terminais ficam montados para manter o PTY vivo.
                for t in group.tabs.iter().filter(|t| matches!(t.kind, TabKind::Terminal { .. })).cloned() {
                    if let TabKind::Terminal { cwd, command, .. } = t.kind {
                        div { key: "{t.id}", class: "pane", style: if group.active == Some(t.id) { "display:block" } else { "display:none" },
                            Terminal { cwd, command }
                        }
                    }
                }
                match active.map(|t| t.kind) {
                    Some(TabKind::Session(id)) => rsx! { div { class: "pane", SessionView { key: "{id}", task: id } } },
                    Some(TabKind::Dashboard) => rsx! { div { class: "pane scroll", Dashboard {} } },
                    Some(TabKind::Metrics) => rsx! { div { class: "pane", MetricsView {} } },
                    None => rsx! {
                        div { class: "pane wb-empty",
                            div { "Nenhuma aba aberta" }
                            div { class: "small muted", "Ctrl+Tab alterna abas · Ctrl+W fecha · Ctrl+\\ divide · arraste abas entre grupos" }
                            div { class: "wb-empty-actions",
                                button { onclick: move |_| { wb.clone().active_group.set(gid); wb.open_dashboard() }, "Painel" }
                                button { onclick: move |_| { wb.clone().active_group.set(gid); wb.open_metrics() }, "Métricas" }
                            }
                        }
                    },
                    _ => rsx! {},
                }
            }
        }
    }
}

const GROUP_EDITOR_JS: &str = r#"
const elId = __EL_ID__;
const base = __BASE__;
function loadScript(src) {
  return new Promise((ok, err) => {
    const s = document.createElement('script');
    s.src = src; s.onload = ok; s.onerror = () => err(new Error('falha ao carregar ' + src));
    document.head.appendChild(s);
  });
}
if (!window.__monacoReady) {
  window.__monacoReady = (async () => {
    await loadScript(base + '/monaco/vs/loader.js');
    window.MonacoEnvironment = { getWorkerUrl: () => base + '/monaco/vs/base/worker/workerMain.js' };
    require.config({ paths: { vs: base + '/monaco/vs' } });
    return await new Promise((ok, err) => {
      const t = setTimeout(() => err(new Error('tempo esgotado ao carregar o Monaco')), 20000);
      require(['vs/editor/editor.main'], () => { clearTimeout(t); ok(window.monaco); }, (e) => { clearTimeout(t); err(e); });
    });
  })();
}
let el = null;
for (let i = 0; i < 200 && !(el = document.getElementById(elId)); i++) await new Promise(r => setTimeout(r, 25));
if (!el) return;
let monaco;
el.textContent = 'Carregando editor…'; el.style.padding = '8px';
try { monaco = await window.__monacoReady; } catch (e) {
  window.__monacoReady = null;
  el.textContent = 'Editor indisponível: ' + (e && e.message ? e.message : e) + ' (base ' + base + ')';
  el.style.padding = '8px'; el.style.color = '#f37272';
  return;
}
el.textContent = ''; el.style.padding = '';
window.__hxModels = window.__hxModels || new Map();
const models = window.__hxModels;
const editor = monaco.editor.create(el, { automaticLayout: true, theme: 'vs-dark', model: null, fontSize: 13 });
let current = null;
editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyS, () => {
  const m = current && models.get(current);
  if (m) dioxus.send({ ev: 'save', path: current, content: m.getValue() });
});
editor.onDidChangeModelContent((e) => { if (current && !e.isFlush) dioxus.send({ ev: 'change', path: current }); });
while (true) {
  const m = await dioxus.recv();
  if (!document.getElementById(elId)) { editor.dispose(); return; }
  if (m.cmd === 'show') {
    let model = models.get(m.path);
    if (!model && m.content != null) { model = monaco.editor.createModel(m.content, m.language); models.set(m.path, model); }
    if (model && current !== m.path) { current = m.path; editor.setModel(model); }
    if (m.focus) editor.focus();
  } else if (m.cmd === 'dispose') {
    const model = models.get(m.path);
    if (current === m.path) { editor.setModel(null); current = null; }
    if (model) { model.dispose(); models.delete(m.path); }
  }
}
"#;

#[derive(Default)]
struct EditorSync {
    shown: Option<PathBuf>,
    /// Arquivos cujo conteúdo este grupo já enviou ao JS.
    sent: HashSet<PathBuf>,
}

/// Instância Monaco de um grupo. Mostra o arquivo da aba ativa do grupo.
#[component]
fn GroupEditor(gid: u64) -> Element {
    let wb = use_workbench();
    let el_id = use_hook(|| unique_dom_id("wb-monaco"));
    let mut error = use_signal(|| None::<String>);
    let sync = use_hook(|| Rc::new(RefCell::new(EditorSync::default())));
    // O eval é criado no `onmounted` (não no primeiro render), garantindo que o
    // webview e o elemento já existam quando o script roda.
    let mut bridge = use_signal(|| None::<document::Eval>);
    let mount = {
        let el_id = el_id.clone();
        move |_| {
            if bridge.peek().is_some() {
                return;
            }
            let js = GROUP_EDITOR_JS
                .replace("__EL_ID__", &json!(el_id).to_string())
                .replace("__BASE__", &json!(assets_base_url()).to_string());
            let ev = document::eval(&js);
            bridge.set(Some(ev.clone()));
            let mut rx = ev;
            spawn(async move {
                while let Ok(msg) = rx.recv::<Value>().await {
                    let p = PathBuf::from(msg["path"].as_str().unwrap_or_default());
                    match msg["ev"].as_str() {
                        Some("change") => {
                            if !wb.dirty.peek().contains(&p) {
                                let mut d = wb.dirty;
                                d.write().insert(p.clone());
                            }
                            wb.pin_file(&p);
                        }
                        Some("save") => match std::fs::write(&p, msg["content"].as_str().unwrap_or_default()) {
                            Ok(()) => {
                                let mut d = wb.dirty;
                                d.write().remove(&p);
                                error.set(None);
                            }
                            Err(e) => error.set(Some(format!("Erro ao salvar {}: {e}", p.display()))),
                        },
                        _ => {}
                    }
                }
            });
        }
    };

    // Sincroniza o arquivo ativo e descarta modelos de arquivos fechados em todos os grupos.
    use_effect({
        let sync = sync.clone();
        move || {
            let Some(bridge) = bridge() else { return };
            let groups = wb.groups.read();
            let open: HashSet<PathBuf> = groups
                .iter()
                .flat_map(|g| g.tabs.iter())
                .filter_map(|t| match &t.kind {
                    TabKind::File(p) => Some(p.clone()),
                    _ => None,
                })
                .collect();
            let want = groups
                .iter()
                .find(|g| g.id == gid)
                .and_then(|g| g.active_tab())
                .and_then(|t| match &t.kind {
                    TabKind::File(p) => Some(p.clone()),
                    _ => None,
                });
            drop(groups);
            let mut s = sync.borrow_mut();
            let closed: Vec<PathBuf> = s.sent.iter().filter(|p| !open.contains(*p)).cloned().collect();
            for p in closed {
                let _ = bridge.send(json!({"cmd": "dispose", "path": p.to_string_lossy()}));
                s.sent.remove(&p);
                if s.shown.as_ref() == Some(&p) {
                    s.shown = None;
                }
            }
            if let Some(p) = want {
                if s.shown.as_ref() != Some(&p) {
                    let content = if s.sent.contains(&p) {
                        None
                    } else {
                        match std::fs::read(&p) {
                            Ok(b) => Some(String::from_utf8_lossy(&b).into_owned()),
                            Err(e) => {
                                error.set(Some(format!("Erro ao abrir {}: {e}", p.display())));
                                return;
                            }
                        }
                    };
                    let _ = bridge.send(json!({
                        "cmd": "show",
                        "path": p.to_string_lossy(),
                        "content": content,
                        "language": language_for(&p),
                        "focus": true,
                    }));
                    s.sent.insert(p.clone());
                    s.shown = Some(p);
                    error.set(None);
                }
            }
        }
    });

    rsx! {
        div { class: "wb-editor",
            if let Some(e) = error() { div { class: "err small wb-err", "{e}" } }
            div { id: "{el_id}", class: "wb-monaco", onmounted: mount }
        }
    }
}

fn state_label(s: &TaskState) -> (&'static str, String) {
    match s {
        TaskState::Queued => ("muted", "Na fila".into()),
        TaskState::Running => ("warn", "Rodando".into()),
        TaskState::Completed => ("ok", "Concluída".into()),
        TaskState::Failed(e) => ("err", format!("Falhou: {e}")),
        TaskState::Cancelled => ("muted", "Cancelada".into()),
        TaskState::Applied => ("ok", "Aplicada".into()),
        TaskState::Discarded => ("muted", "Descartada".into()),
    }
}

/// Chat de uma tarefa: histórico + stream ao vivo + resposta (`Orchestrator::reply`).
#[component]
pub fn SessionView(task: TaskId) -> Element {
    let app = use_app();
    let orch = use_orchestrator();
    let mut events = use_signal(Vec::<AgentEvent>::new);
    let mut draft = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    let log_id = use_hook(|| unique_dom_id("wb-chat"));

    use_future({
        let orch = orch.clone();
        move || {
            let orch = orch.clone();
            async move {
                let mut rx = orch.subscribe();
                let mut hist = orch.history(task);
                if hist.is_empty() {
                    // Sem banco: usa o log em texto do app.
                    if let Some(lines) = app.task_log.peek().get(&task) {
                        hist = lines.iter().cloned().map(AgentEvent::Raw).collect();
                    }
                }
                events.set(hist);
                loop {
                    match rx.recv().await {
                        Ok(OrchestratorEvent::Agent { task: t, event }) if t == task => events.write().push(event),
                        Ok(OrchestratorEvent::Stderr { task: t, line }) if t == task => events.write().push(AgentEvent::Raw(line)),
                        Ok(_) => {}
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                        Err(_) => break,
                    }
                }
            }
        }
    });

    // Rola para o fim quando chegam eventos.
    use_effect({
        let log_id = log_id.clone();
        move || {
            let _n = events.read().len();
            document::eval(&format!(
                "const e=document.getElementById({}); if(e) e.scrollTop=e.scrollHeight;",
                json!(log_id)
            ));
        }
    });

    let _rev = (app.task_rev)();
    let info = orch.task(task);
    let busy = info.as_ref().is_some_and(|t| t.state.is_active());

    let send = {
        let orch = orch.clone();
        move || {
            let msg = draft.peek().trim().to_string();
            if msg.is_empty() {
                return;
            }
            match orch.reply(task, &msg) {
                Ok(()) => {
                    draft.set(String::new());
                    error.set(None);
                }
                Err(e) => error.set(Some(e.to_string())),
            }
        }
    };
    let send_btn = send.clone();
    let orch_cancel = orch.clone();

    let items = events.read().clone();
    rsx! {
        div { class: "wb-chat",
            div { class: "wb-chat-head",
                match &info {
                    Some(t) => {
                        let (cls, label) = state_label(&t.state);
                        let proj = t.project.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                        rsx! {
                            div { class: "ellipsis", title: "{t.prompt}", b { "#{task} " } "{t.prompt}" }
                            div { class: "small muted", "{proj} · {t.provider} · turno {t.turns} " span { class: "{cls}", "{label}" } }
                            crate::ui::autowork::AutoWorkBadge { task }
                            super::servers::SessionServersBadge { task }
                        }
                    }
                    None => rsx! { div { class: "muted", "Tarefa #{task} não encontrada" } },
                }
                if busy {
                    button { onclick: move |_| { let _ = orch_cancel.cancel(task); super::servers::confirm_stop_session(task); }, "Cancelar" }
                }
            }
            div { id: "{log_id}", class: "wb-chat-log",
                for (i, ev) in items.into_iter().enumerate() {
                    ChatItem { key: "{i}", ev }
                }
                if busy { div { class: "muted small wb-typing", "o agente está trabalhando\u{2026}" } }
            }
            if let Some(e) = error() { div { class: "err small", style: "padding:0 10px", "{e}" } }
            div { class: "wb-chat-input",
                textarea {
                    rows: "3",
                    placeholder: if busy { "Aguarde o turno terminar\u{2026}" } else { "Responder ao agente (Ctrl+Enter envia)" },
                    value: "{draft}",
                    oninput: move |e| draft.set(e.value()),
                    onkeydown: {
                        let mut send = send.clone();
                        move |e: KeyboardEvent| {
                            if e.key() == Key::Enter && e.modifiers().ctrl() {
                                e.prevent_default();
                                send();
                            }
                        }
                    },
                }
                button {
                    class: "primary",
                    disabled: busy || draft.read().trim().is_empty(),
                    onclick: { let mut s = send_btn.clone(); move |_| s() },
                    "Enviar"
                }
            }
        }
    }
}

#[component]
fn ChatItem(ev: AgentEvent) -> Element {
    match ev {
        AgentEvent::Session(_) => rsx! {},
        AgentEvent::User(t) => rsx! { div { class: "wb-msg user", "{t}" } },
        AgentEvent::Text(t) => rsx! { div { class: "wb-msg agent", "{t}" } },
        AgentEvent::ToolUse { name, input } => {
            let detail = serde_json::to_string_pretty(&input).unwrap_or_default();
            rsx! {
                details { class: "wb-tool",
                    summary { span { class: "wb-ic", "\u{2699}" } " {name}" }
                    pre { "{detail}" }
                }
            }
        }
        AgentEvent::Result { ok, summary, cost } => {
            let c = cost.map(|c| format!(" · ${c:.4}")).unwrap_or_default();
            rsx! {
                div { class: if ok { "wb-result ok" } else { "wb-result err" },
                    if ok { "\u{2713} concluído{c}" } else { "\u{2717} erro{c}" }
                    if !summary.is_empty() && !ok { div { class: "small", "{summary}" } }
                }
            }
        }
        AgentEvent::Usage(u) => {
            let model = u.model.clone().unwrap_or_default();
            rsx! {
                div { class: "small muted wb-usage",
                    "tokens: {u.input_tokens} entrada · {u.output_tokens} saída · cache {u.cache_read} lido · {u.cache_write} gravado {model}"
                }
            }
        }
        AgentEvent::Raw(r) => notice_item(&r),
    }
}

/// Resumo de uma linha de aviso (stderr/sistema): primeira linha, até 90 caracteres.
pub fn notice_summary(raw: &str) -> String {
    let first = raw.trim().lines().next().unwrap_or_default().trim();
    let first = first.strip_prefix("[stderr]").unwrap_or(first).trim();
    let mut s: String = first.chars().take(90).collect();
    if first.chars().count() > 90 || raw.trim().lines().nth(1).is_some() {
        s.push('\u{2026}');
    }
    s
}

/// Avisos crus da CLI (ex.: "MCP servers blocked by enterprise policy") exibidos
/// discretamente: cinza e colapsados, com o texto completo ao expandir.
fn notice_item(raw: &str) -> Element {
    let raw = raw.trim().to_string();
    if raw.is_empty() {
        return rsx! {};
    }
    let summary = notice_summary(&raw);
    rsx! {
        details { class: "wb-notice",
            summary { span { class: "wb-ic", "\u{24D8}" } " {summary}" }
            pre { "{raw}" }
        }
    }
}

const WB_CSS: &str = r#"
.wb-notice { color: #7d8290; font-size: 11px; margin: 2px 0; }
.wb-notice summary { cursor: pointer; list-style: none; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; opacity: .75; }
.wb-notice summary::-webkit-details-marker { display: none; }
.wb-notice[open] summary { opacity: 1; }
.wb-notice pre { margin: 4px 0 0; padding: 6px; background: #111317; border-radius: 4px; white-space: pre-wrap; word-break: break-word; font: 11px Consolas, monospace; color: #8b90a0; }
.wb { display: flex; flex-direction: column; height: 100%; min-height: 0; }
.wb-main { flex: 1; min-height: 0; display: flex; }
.wb-activity { width: 46px; flex: none; background: #14161a; display: flex; flex-direction: column; align-items: center; padding-top: 4px; border-right: 1px solid #2a2e37; }
.wb-act { width: 46px; height: 46px; background: none; border: 0; border-left: 2px solid transparent; border-radius: 0; color: #8b90a0; display: flex; align-items: center; justify-content: center; padding: 0; }
.wb-act:hover { color: #d6d9e0; background: none; }
.wb-act.active { color: #fff; border-left-color: #3b6ef5; }
.wb-side { width: 270px; flex: none; background: #1d2027; border-right: 1px solid #2a2e37; display: flex; flex-direction: column; min-height: 0; }
.wb-side-title { padding: 8px 12px; font-size: 11px; text-transform: uppercase; letter-spacing: .04em; opacity: .7; flex: none; }
.wb-side-body { flex: 1; min-height: 0; overflow: auto; }
.wb-center { flex: 1; min-width: 0; min-height: 0; display: flex; flex-direction: column; }
.wb-groups { flex: 1; min-width: 0; min-height: 0; display: flex; }
.wb-groups.wb-row { flex-direction: row; } .wb-groups.wb-col { flex-direction: column; }
.wb-group { flex: 1 1 0; min-width: 0; min-height: 0; display: flex; flex-direction: column; border-left: 1px solid #2a2e37; border-top: 1px solid #2a2e37; }
.wb-group:first-child { border-left: 0; border-top: 0; }
.wb-group.focused .wb-tab.active { border-top-color: #3b6ef5; }
.wb-tabs { display: flex; background: #1a1d23; overflow-x: auto; flex: none; min-height: 34px; }
.wb-tabs::-webkit-scrollbar { height: 3px; }
.wb-tab { display: flex; align-items: center; gap: 6px; padding: 0 6px 0 10px; height: 34px; cursor: pointer; white-space: nowrap; border-right: 1px solid #2a2e37; border-top: 1px solid transparent; color: #9aa0aa; user-select: none; }
.wb-tab:hover { background: #20242c; }
.wb-tab.active { background: #1e1e1e; color: #e6e8ee; border-top-color: #555c6a; }
.wb-tab.preview .wb-title { font-style: italic; }
.wb-ic { font: 600 10px Consolas, monospace; min-width: 14px; text-align: center; }
.wb-x { width: 18px; height: 18px; display: inline-flex; align-items: center; justify-content: center; border-radius: 4px; }
.wb-x:hover { background: #333845; }
.wb-x .wb-dot { display: none; font-size: 10px; }
.wb-x .wb-cross { visibility: hidden; }
.wb-tab:hover .wb-cross, .wb-tab.active .wb-cross { visibility: visible; }
.wb-tab.dirty .wb-dot { display: inline; }
.wb-tab.dirty .wb-cross { display: none; }
.wb-tab.dirty .wb-x:hover .wb-dot { display: none; }
.wb-tab.dirty .wb-x:hover .wb-cross { display: inline; visibility: visible; }
.wb-tabs-fill { flex: 1; min-width: 20px; }
.wb-tab-actions { display: flex; align-items: center; gap: 2px; padding: 0 4px; flex: none; }
.wb-sbtn { background: none; border: 0; padding: 2px 6px; color: #9aa0aa; font-size: 12px; }
.wb-sbtn:hover { background: #2a2e37; color: #fff; }
.wb-body { flex: 1; min-height: 0; position: relative; background: #1e1e1e; }
.wb-editor { position: absolute; inset: 0; display: flex; flex-direction: column; }
.wb-monaco { flex: 1; min-height: 0; }
.wb-err { padding: 4px 8px; }
.wb-empty { display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 8px; color: #8b90a0; }
.wb-empty-actions { display: flex; gap: 6px; }
.wb-status { height: 24px; flex: none; display: flex; justify-content: space-between; align-items: center; background: #1f3a75; color: #e6ecff; font-size: 12px; padding: 0 6px; gap: 8px; }
.wb-status-l, .wb-status-r { display: flex; align-items: center; gap: 10px; min-width: 0; }
.wb-status .wb-sbtn { color: #e6ecff; }
.wb-chat { position: absolute; inset: 0; display: flex; flex-direction: column; }
.wb-chat-head { display: flex; justify-content: space-between; align-items: center; gap: 10px; padding: 8px 12px; border-bottom: 1px solid #2a2e37; flex: none; min-width: 0; }
.wb-chat-head > div:first-child { min-width: 0; }
.wb-chat-log { flex: 1; min-height: 0; overflow: auto; padding: 12px; display: flex; flex-direction: column; gap: 8px; }
.wb-msg { padding: 8px 10px; border-radius: 8px; white-space: pre-wrap; max-width: 85%; line-height: 1.45; }
.wb-msg.user { align-self: flex-end; background: #2b3550; }
.wb-msg.agent { align-self: flex-start; background: #23262e; }
.wb-tool { align-self: flex-start; font-size: 12px; color: #9aa0aa; max-width: 85%; }
.wb-tool summary { cursor: pointer; }
.wb-tool pre { background: #111317; padding: 6px; border-radius: 4px; white-space: pre-wrap; max-height: 240px; overflow: auto; margin: 4px 0 0; }
.wb-result { font-size: 12px; }
.wb-raw { font: 11px Consolas, monospace; color: #8b90a0; white-space: pre-wrap; }
.wb-usage { align-self: flex-start; }
.wb-chat-input { display: flex; gap: 8px; padding: 8px 12px; border-top: 1px solid #2a2e37; flex: none; align-items: flex-end; }
.wb-chat-input textarea { flex: 1; resize: vertical; }
"#;

#[cfg(test)]
mod tests {
    use super::*;

    fn tab(id: u64, f: &str) -> WbTab {
        WbTab { id, kind: TabKind::File(PathBuf::from(f)), preview: false }
    }
    fn ids(g: &Group) -> Vec<u64> {
        g.tabs.iter().map(|t| t.id).collect()
    }

    #[test]
    fn cycle_wraps_both_ways() {
        let mut g = Group { id: 0, tabs: vec![tab(1, "a"), tab(2, "b"), tab(3, "c")], active: Some(3) };
        cycle_in(&mut g, false);
        assert_eq!(g.active, Some(1));
        cycle_in(&mut g, true);
        assert_eq!(g.active, Some(3));
        let mut empty = Group { id: 1, tabs: vec![], active: None };
        cycle_in(&mut empty, false);
        assert_eq!(empty.active, None);
    }

    #[test]
    fn move_between_groups_and_reorder() {
        let mut gs = vec![
            Group { id: 0, tabs: vec![tab(1, "a"), tab(2, "b")], active: Some(1) },
            Group { id: 9, tabs: vec![tab(3, "c")], active: Some(3) },
        ];
        assert!(move_tab_in(&mut gs, 1, 9, Some(3)));
        assert_eq!(ids(&gs[0]), [2]);
        assert_eq!(gs[0].active, Some(2));
        assert_eq!(ids(&gs[1]), [1, 3]);
        assert_eq!(gs[1].active, Some(1));
        // Reordena no mesmo grupo (para o fim).
        assert!(move_tab_in(&mut gs, 1, 9, None));
        assert_eq!(ids(&gs[1]), [3, 1]);
        // Último da origem: grupo some.
        assert!(move_tab_in(&mut gs, 2, 9, None));
        assert_eq!(gs.len(), 1);
        assert_eq!(ids(&gs[0]), [3, 1, 2]);
        // Soltar sobre si mesma: nada muda.
        assert!(!move_tab_in(&mut gs, 2, 9, Some(2)));
    }

    #[test]
    fn move_duplicate_file_focuses_existing() {
        let mut gs = vec![
            Group { id: 0, tabs: vec![tab(1, "a"), tab(2, "b")], active: Some(1) },
            Group { id: 5, tabs: vec![tab(3, "a")], active: Some(3) },
        ];
        assert!(move_tab_in(&mut gs, 1, 5, None));
        assert_eq!(ids(&gs[0]), [2]);
        assert_eq!(ids(&gs[1]), [3]);
        assert_eq!(gs[1].active, Some(3));
    }

    #[test]
    fn notice_summary_trims() {
        assert_eq!(notice_summary("[stderr] MCP servers blocked by enterprise policy"), "MCP servers blocked by enterprise policy");
        assert!(notice_summary("linha1\nlinha2").ends_with('\u{2026}'));
    }
}
