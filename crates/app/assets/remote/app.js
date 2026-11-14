"use strict";
const $ = (s) => document.querySelector(s);
let tasks = [];
let current = null;
const awRuns = {};
let awSettings = {};

function awLabel(r) {
  if (!r) return "";
  const s = stateName(r.status);
  if (s === "Running") return `auto ${r.done}/${r.total} · it ${r.iteration}/${r.max_iterations}`;
  if (s === "Paused") return `auto pausado ${r.done}/${r.total}`;
  if (s === "Completed") return `auto concluído ${r.done}/${r.total}`;
  return `auto parado: ${r.status.Stopped}`;
}

async function loadAutowork() {
  try {
    const d = await api("/api/autowork");
    awSettings = {};
    d.settings.forEach((x) => (awSettings[x.project] = x.settings));
    d.runs.forEach((r) => (awRuns[r.task] = r));
  } catch (_) { /* auto-work indisponível */ }
}

function renderAw() {
  const t = tasks.find((x) => x.id === current);
  if (!t) return;
  const r = awRuns[t.id];
  const s = awSettings[t.project];
  $("#aw-enabled").checked = !!(s && s.enabled);
  const st = r ? stateName(r.status) : null;
  const pill = $("#aw-status");
  pill.className = "aw-pill " + (st || "");
  pill.textContent = awLabel(r);
  $("#aw-item").textContent = r && r.current_item ? r.current_item : "";
  $("#aw-pause").hidden = st !== "Running";
  $("#aw-resume").hidden = st !== "Paused";
  $("#aw-stop").hidden = !(st === "Running" || st === "Paused");
  $("#aw-start").hidden = st === "Running" || st === "Paused";
}

async function awAction(a) {
  if (current == null) return;
  await api(`/api/tasks/${current}/autowork/${a}`, { method: "POST" }).catch((e) => alert(e.message));
}

async function api(path, opts = {}) {
  const r = await fetch(path, { credentials: "same-origin", headers: { "Content-Type": "application/json" }, ...opts });
  if (r.status === 401) {
    document.body.innerHTML = "<p style='padding:16px'>Acesso revogado. Escaneie o QR code novamente na IDE.</p>";
    throw new Error("401");
  }
  const txt = await r.text();
  const data = txt ? JSON.parse(txt) : null;
  if (!r.ok) throw new Error((data && data.error) || r.statusText);
  return data;
}

const stateName = (s) => (typeof s === "string" ? s : Object.keys(s)[0]);
const LABELS = { Queued: "na fila", Running: "rodando", Completed: "concluída", Failed: "falhou", Cancelled: "cancelada", Applied: "aplicada", Discarded: "descartada" };
const stateLabel = (s) => LABELS[stateName(s)] || stateName(s);
const base = (p) => String(p).split(/[\\/]/).filter(Boolean).pop();

function renderList() {
  const ul = $("#tasks");
  ul.innerHTML = "";
  if (!tasks.length) ul.innerHTML = "<li class='m sys'>Nenhuma sessão ainda.</li>";
  [...tasks].reverse().forEach((t) => {
    const li = document.createElement("li");
    li.className = "task";
    li.innerHTML = '<div class="top"><span class="h"></span><span class="st"></span></div><div class="p"></div>';
    li.querySelector(".h").textContent = `#${t.id} · ${base(t.project)} · ${t.provider}`;
    const st = li.querySelector(".st");
    st.classList.add(stateName(t.state));
    st.textContent = stateLabel(t.state);
    li.querySelector(".p").textContent = t.prompt;
    if (awRuns[t.id]) {
      const b = document.createElement("span");
      b.className = "aw-pill " + stateName(awRuns[t.id].status);
      b.textContent = awLabel(awRuns[t.id]);
      li.querySelector(".top .h").after(b);
    }
    li.onclick = () => openTask(t.id);
    ul.appendChild(li);
  });
  const dl = $("#projects");
  dl.innerHTML = "";
  [...new Set(tasks.map((t) => t.project))].forEach((p) => {
    const o = document.createElement("option");
    o.value = p;
    dl.appendChild(o);
  });
}

async function loadList() {
  tasks = await api("/api/tasks");
  await loadAutowork();
  renderList();
}

function addEvent(ev) {
  const div = document.createElement("div");
  const [kind, val] = typeof ev === "string" ? [ev, null] : Object.entries(ev)[0];
  switch (kind) {
    case "User": div.className = "m user"; div.textContent = val; break;
    case "Text": div.className = "m text"; div.textContent = val; break;
    case "ToolUse": div.className = "m tool"; div.textContent = `> ${val.name} ${JSON.stringify(val.input).slice(0, 300)}`; break;
    case "Result":
      div.className = "m result" + (val.ok ? "" : " bad");
      div.textContent = (val.summary || (val.ok ? "concluído" : "erro")) + (val.cost ? `\nUS$ ${val.cost.toFixed(4)}` : "");
      break;
    case "Usage":
      div.className = "m sys";
      div.textContent = `tokens: ${val.input_tokens} entrada / ${val.output_tokens} saída` + (val.cost_usd ? ` · US$ ${val.cost_usd.toFixed(4)}` : "");
      break;
    case "Session": return;
    default: div.className = "m sys"; div.textContent = typeof val === "string" ? val : JSON.stringify(val);
  }
  const atBottom = window.innerHeight + window.scrollY >= document.body.scrollHeight - 40;
  $("#log").appendChild(div);
  if (atBottom) window.scrollTo(0, document.body.scrollHeight);
}

function updateChrome() {
  const t = tasks.find((x) => x.id === current);
  if (!t) return;
  $("#title").textContent = `#${t.id} ${base(t.project)}`;
  $("#meta").textContent = `${t.provider} · ${stateLabel(t.state)} · ${t.project}`;
  const active = ["Queued", "Running"].includes(stateName(t.state));
  $("#cancel").hidden = !active;
  $("#msg").disabled = active;
  $("#msg").placeholder = active ? "Aguardando o agente..." : "Responder...";
}

async function openTask(id) {
  current = id;
  if (location.hash !== "#" + id) history.pushState(null, "", "#" + id);
  $("#list-view").hidden = true;
  $("#chat-view").hidden = false;
  $("#reply").hidden = false;
  $("#back").hidden = false;
  $("#log").innerHTML = "";
  const d = await api(`/api/tasks/${id}`);
  const i = tasks.findIndex((t) => t.id === id);
  if (i >= 0) tasks[i] = d.task; else tasks.push(d.task);
  d.events.forEach(addEvent);
  updateChrome();
  renderAw();
  window.scrollTo(0, document.body.scrollHeight);
}

function showList() {
  current = null;
  $("#list-view").hidden = false;
  $("#chat-view").hidden = true;
  $("#reply").hidden = true;
  $("#back").hidden = true;
  $("#title").textContent = "Sessões";
  loadList().catch(() => {});
}

$("#back").onclick = () => { history.pushState(null, "", location.pathname); showList(); };
window.onpopstate = () => {
  const id = parseInt(location.hash.slice(1), 10);
  if (id) openTask(id); else showList();
};

$("#reply").onsubmit = async (e) => {
  e.preventDefault();
  const msg = $("#msg").value.trim();
  if (!msg || current == null) return;
  try {
    await api(`/api/tasks/${current}/reply`, { method: "POST", body: JSON.stringify({ message: msg }) });
    $("#msg").value = "";
  } catch (err) {
    alert("Não foi possível enviar: " + err.message);
  }
};
$("#msg").addEventListener("keydown", (e) => {
  if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) $("#reply").requestSubmit();
});
$("#cancel").onclick = async () => {
  if (current != null && confirm("Parar esta tarefa?")) {
    await api(`/api/tasks/${current}/cancel`, { method: "POST" }).catch((e) => alert(e.message));
  }
};

$("#aw-pause").onclick = () => awAction("pause");
$("#aw-resume").onclick = () => awAction("resume");
$("#aw-start").onclick = () => awAction("start");
$("#aw-stop").onclick = () => { if (confirm("Parar o auto-work desta sessão?")) awAction("stop"); };
$("#aw-enabled").onchange = async (ev) => {
  const t = tasks.find((x) => x.id === current);
  if (!t) return;
  try {
    const r = await api("/api/autowork", { method: "POST", body: JSON.stringify({ project: t.project, enabled: ev.target.checked }) });
    awSettings[t.project] = r.settings;
  } catch (err) { alert(err.message); ev.target.checked = !ev.target.checked; }
};

$("#new-form").onsubmit = async (e) => {
  e.preventDefault();
  try {
    const body = { project: $("#nf-project").value, provider: $("#nf-provider").value, prompt: $("#nf-prompt").value };
    const r = await api("/api/tasks", { method: "POST", body: JSON.stringify(body) });
    $("#nf-prompt").value = "";
    $("#new").open = false;
    await loadList();
    openTask(r.id);
  } catch (err) {
    alert("Erro: " + err.message);
  }
};

function connect() {
  const ws = new WebSocket((location.protocol === "https:" ? "wss://" : "ws://") + location.host + "/api/stream");
  ws.onopen = () => $("#conn").classList.add("on");
  ws.onclose = () => {
    $("#conn").classList.remove("on");
    setTimeout(connect, 3000);
  };
  ws.onmessage = (m) => {
    const e = JSON.parse(m.data);
    if (e.type === "resync") {
      if (current != null) openTask(current); else loadList();
      return;
    }
    if (e.type === "autowork") {
      awRuns[e.task] = e.autowork;
      if (current === e.task) renderAw(); else if (current == null) renderList();
      return;
    }
    if (e.type === "state") {
      const t = tasks.find((x) => x.id === e.task);
      if (t) t.state = e.state; else loadList();
      if (current === e.task) updateChrome(); else if (current == null) renderList();
    } else if (e.type === "agent" && current === e.task) {
      addEvent(e.event);
    } else if (e.type === "stderr" && current === e.task) {
      addEvent({ Raw: e.line });
    }
  };
}

connect();
const start = parseInt(location.hash.slice(1), 10);
loadList().then(() => { if (start) openTask(start); }).catch(() => {});

// ---- Servidores vinculados às sessões (GET /api/servers, POST /api/servers/:pid/kill)
function ownerLabel(o) {
  return o.kind === "Session" ? `Sessão #${o.id}` : `Terminal ${o.id}`;
}
async function loadServers() {
  const ul = document.getElementById("srv-list");
  if (!ul) return;
  let list = [];
  try { list = await api("/api/servers"); } catch { return; }
  document.getElementById("srv-count").textContent = `(${list.length})`;
  ul.replaceChildren();
  if (!list.length) {
    const li = document.createElement("li");
    li.className = "muted";
    li.textContent = "Nenhum servidor rodando.";
    ul.append(li);
  }
  for (const s of list) {
    const li = document.createElement("li");
    li.className = "srv";
    const info = document.createElement("div");
    const b = document.createElement("b");
    b.textContent = `:${s.port}`;
    const meta = document.createElement("small");
    meta.textContent = ` ${ownerLabel(s.owner)} · ${s.name} · pid ${s.pid}`;
    info.append(b, meta);
    const cmd = document.createElement("small");
    cmd.className = "srv-cmd";
    cmd.textContent = s.cmdline;
    const kill = document.createElement("button");
    kill.className = "danger";
    kill.textContent = "Derrubar";
    kill.onclick = async () => {
      if (!confirm(`Derrubar o servidor na porta ${s.port}?`)) return;
      await api(`/api/servers/${s.pid}/kill`, { method: "POST" }).catch((e) => alert(e.message));
      loadServers();
    };
    li.append(info, cmd, kill);
    ul.append(li);
  }
}
loadServers();
setInterval(loadServers, 5000);