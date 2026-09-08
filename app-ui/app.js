const $ = (id) => document.getElementById(id);
let token = location.hash.slice(1) || sessionStorage.getItem("portrelay-token") || "";
if (location.hash) { sessionStorage.setItem("portrelay-token", token); history.replaceState(null, "", "/"); }
let state, selectedPeer = "", view = "local", busy = false;
function node(tag, text, className) { const e = document.createElement(tag); if (text !== undefined) e.textContent = text; if (className) e.className = className; return e; }
function tell(message, error = false) { $("message").textContent = message; $("message").classList.toggle("error", error); }
async function request(path, data) {
  const response = await fetch(path, {method:data ? "POST" : "GET", headers:{Authorization:`Bearer ${token}`, ...(data ? {"Content-Type":"application/json"} : {})}, ...(data ? {body:JSON.stringify(data)} : {})});
  const result = await response.json(); if (!response.ok) throw new Error(result.error || "The request failed."); return result;
}
async function action(data) { const result = await request("/api/action", data); if (result.message) tell(result.message); return result; }
function button(label, task, secondary = false) { const b = node("button", label, secondary ? "secondary" : ""); b.onclick = () => run(task); return b; }
async function run(task) { if (busy) return; busy = true; try { await task(); await refresh(); } catch (e) { tell(e.message, true); } finally { busy = false; } }
function empty(container, text) { container.replaceChildren(node("p", text, "empty")); }
function renderPeers() {
  const entries = Object.entries(state.peers);
  if (!entries.some(([id]) => id === selectedPeer)) selectedPeer = entries.find(([,p]) => p.approved)?.[0] || "";
  const container = $("peers"); container.replaceChildren();
  for (const [id, peer] of entries) {
    const card = node("div", undefined, `peer-card${id === selectedPeer ? " selected" : ""}`);
    card.append(node("h3", peer.name), node("p", peer.approved ? "Paired computer" : "Waiting for your approval", "muted"), node("p", id, "fingerprint"));
    if (!peer.approved) card.append(button("Approve", () => action({op:"approve", peer:id})));
    else card.append(button(id === selectedPeer ? "Selected" : "Select", async () => {selectedPeer = id;}, true));
    card.append(button("Remove", async () => {if (confirm(`Remove trust for ${peer.name} and close its connections?`)) await action({op:"revoke", peer:id});}, true));
    container.append(card);
  }
  if (!entries.length) empty(container, "Pair your other computer to get started. Devices stay private until you share them.");
}
async function renderDevices() {
  const container = $("device-list");
  let devices = state.devices;
  if (view === "remote") {
    if (!selectedPeer) {empty(container, "Choose a paired computer first."); return;}
    try {devices = (await action({op:"remote", peer:selectedPeer})).devices;}
    catch (e) {empty(container, e.message); return;}
  }
  container.replaceChildren();
  for (const device of devices) {
    const row = node("div", undefined, "device-row"), details = node("div", undefined, "device-details");
    row.append(node("span", device.kind === "bluetooth" ? "ᛒ" : "↔", "device-icon"));
    const grant = state.grants[device.id];
    const shared = grant?.generation === device.generation && grant.peers.includes(selectedPeer);
    details.append(node("h3", device.name), node("p", `${device.id} · ${device.kind === "bluetooth" ? "Whole Bluetooth adapter" : "USB"} · ${device.blocked || (view === "local" ? shared ? "Shared with selected computer" : "Private" : device.busy ? "In use" : "Available")}`));
    row.append(details);
    if (view === "local") {
      const b = button(shared ? "Stop sharing" : "Share", async () => {
        if (!selectedPeer) throw new Error("Pair and select a computer before sharing.");
        if (shared) await action({op:"unshare", device:device.id});
        else {
          let acknowledge_bluetooth = false;
          if (device.kind === "bluetooth") {acknowledge_bluetooth = confirm("Use a dedicated Bluetooth adapter. Connecting it remotely disconnects its local Bluetooth devices. Its radio stays near this computer. Continue?"); if (!acknowledge_bluetooth) return;}
          await action({op:"share", device:device.id, peer:selectedPeer, acknowledge_bluetooth});
        }
      }, shared);
      b.disabled = !!device.blocked || !state.helper_ready || !selectedPeer; row.append(b);
    } else {
      const active = state.sessions.find(s => s.direction === "incoming" && s.device === device.id && s.peer === selectedPeer);
      const b = button(active ? "Disconnect" : "Connect", () => active ? action({op:"disconnect", session:active.id}) : action({op:"connect", peer:selectedPeer, device:device.id, generation:device.generation}), !!active);
      b.disabled = (!active && device.busy) || !state.helper_ready; row.append(b);
    }
    container.append(row);
  }
  if (!devices.length) empty(container, view === "local" ? "No USB devices found on this computer." : "No devices shared yet. On the other computer, select this computer and choose Share.");
}
function renderSessions() {
  const container = $("sessions"); container.replaceChildren();
  for (const session of state.sessions) {
    const row = node("div", undefined, "session"), info = node("div");
    info.append(node("strong", `${session.device} · ${session.state}`), node("p", `${session.direction === "outgoing" ? "Lending to" : "Using from"} ${state.peers[session.peer]?.name || session.peer.slice(0,12)}`));
    row.append(info, button("Disconnect", () => action({op:"disconnect", session:session.id}), true));container.append(row);
  }
  if (!state.sessions.length) empty(container, "No active device connections.");
  for (const session of state.history.slice(0,3).filter(s => s.error)) container.append(node("p", `${session.device}: ${session.error}`, "notice"));
}
async function refresh() {
  state = await request("/api/state");
  $("computer-name").textContent = state.name; $("network").textContent = state.network;
  $("version").textContent = `PortRelay ${state.version}`; $("identity").textContent = `Identity ${state.id.slice(0,12)}…`; $("identity").title = state.id;
  $("setup").hidden = state.helper_ready;
  $("setup").textContent = state.platform !== "linux" ? "This computer can pair and manage peers. Real USB attachment currently requires Linux on both computers." : (state.helper_error || "Install the Linux device helper using the included installer. See Source & help below.");
  renderPeers(); await renderDevices(); renderSessions();
}
$("invite").onclick = () => run(async () => {$("pairing").hidden = !$("pairing").hidden; if (!$("pairing").hidden) $("invitation-out").value = (await action({op:"invite"})).invitation;});
$("copy-invitation").onclick = () => run(async () => {await navigator.clipboard.writeText($("invitation-out").value); tell("Invitation copied. Send it privately to your other computer.");});
$("pair").onclick = () => run(async () => {await action({op:"pair", invitation:$("invitation-in").value}); $("invitation-in").value = "";});
$("refresh").onclick = () => run(async () => {});
for (const v of ["local", "remote"]) $(v + "-tab").onclick = () => run(async () => {view = v; for (const key of ["local","remote"]) {$(key+"-tab").classList.toggle("active",key===view);$(key+"-tab").setAttribute("aria-pressed",String(key===view));}});
refresh().then(() => tell("Ready. Pair a computer, share one device, and connect from the other side.")).catch(e => tell(e.message, true));
setInterval(() => {if (!busy && document.visibilityState === "visible") run(async () => {});}, 5000);
