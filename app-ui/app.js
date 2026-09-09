const $ = (id) => document.getElementById(id);
const token = location.hash.slice(1) || sessionStorage.getItem("portrelay-token") || "";
if (location.hash) {
  sessionStorage.setItem("portrelay-token", token);
  history.replaceState(null, "", "/");
}
let state, selectedPeer = "", view = "local", busy = false;
let refreshRevision = 0, backgroundRefreshing = false;
let currentInvitation = "", invitationExpiry = 0;
const screens = {
  computers: ["Computers", "All your computers, in one place."],
  devices: ["USB devices", "Share from here, or use a device from another computer."],
  input: ["Keyboard & mouse", "Use this keyboard and mouse on another computer."],
  connections: ["Connections", "See what is in use and return devices to their owners."],
  settings: ["Settings", "Your account, computer name, and one-time setup."],
};
let screen = "computers", screenInitialized = false;
try { const saved = sessionStorage.getItem("portrelay-screen"); if (Object.hasOwn(screens, saved)) screen = saved; } catch {}
const deviceNames = new Map();
const labels = { connecting: "Connecting…", connected: "Connected", restoring: "Returning to its owner…", detaching: "Disconnecting…" };
function node(tag, text, className) {
  const element = document.createElement(tag);
  if (text !== undefined) element.textContent = text;
  if (className) element.className = className;
  return element;
}
function tell(message, error = false) {
  $("message").textContent = message;
  $("message").classList.toggle("error", error);
}
function confirmAction(message, title = "Confirm action", label = "Continue") {
  const dialog = $("confirm-dialog");
  if (dialog.open) return Promise.resolve(false);
  $("confirm-title").textContent = title;
  $("confirm-message").textContent = message;
  $("confirm-accept").textContent = label;
  return new Promise(resolve => {
    const closed = () => {
      dialog.removeEventListener("close", closed);
      resolve(dialog.returnValue === "confirmed");
    };
    dialog.returnValue = "cancelled";
    dialog.addEventListener("close", closed);
    dialog.showModal();
    $("confirm-cancel").focus();
  });
}
$("confirm-cancel").onclick = () => $("confirm-dialog").close("cancelled");
$("confirm-accept").onclick = () => $("confirm-dialog").close("confirmed");
async function request(path, data) {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), data?.op === "input_events" ? 2500 : data?.op === "connect" ? 60000 : 25000);
  try {
    const response = await fetch(path, {
      method: data ? "POST" : "GET", signal: controller.signal,
      headers: { Authorization: `Bearer ${token}`, ...(data ? { "Content-Type": "application/json" } : {}) },
      ...(data ? { body: JSON.stringify(data) } : {}),
    });
    const result = await response.json();
    if (!response.ok) throw new Error(result.error || "The request failed. Try again.");
    return result;
  } catch (error) {
    if (error.name === "AbortError") throw new Error("This is taking too long. Check that PortRelay is open on the other computer, then try again.");
    throw error;
  } finally { clearTimeout(timeout); }
}
async function action(data) {
  const result = await request("/api/action", data);
  if (result.message) tell(result.message);
  return result;
}
function button(label, task, id, secondary = false) {
  const b = node("button", label, secondary ? "secondary" : "");
  b.id = id;
  b.onclick = () => run(task, b);
  return b;
}
async function run(task, control) {
  if (busy) return;
  tell("");
  busy = true;
  refreshRevision += 1;
  if (control) { control.disabled = true; control.setAttribute("aria-busy", "true"); }
  try { await task(); await refresh(); }
  catch (error) { tell(error.message, true); }
  finally { busy = false; if (control?.isConnected) { control.disabled = control.id === "enable-usb" && !!state?.setup?.running; control.removeAttribute("aria-busy"); } }
}
function empty(container, text) { container.replaceChildren(node("p", text, "empty")); }
function peerName(id) { return state.peers[id]?.name || "Other computer"; }
function setView(next) {
  view = next;
  for (const key of ["local", "remote"]) {
    $(key + "-tab").classList.toggle("active", key === view);
    $(key + "-tab").setAttribute("aria-pressed", String(key === view));
  }
}
function setPairMode(create) {
  $("create-panel").hidden = !create;
  $("join-panel").hidden = create;
  $("create-mode").classList.toggle("active", create);
  $("join-mode").classList.toggle("active", !create);
}
function renderSetup() {
  const approved = Object.values(state.peers).some((p) => p.approved);
  $("name-setup").hidden = false;
  $("usb-setup-notice").hidden = state.helper_ready;
  $("setup-dot").hidden = state.helper_ready && state.account?.status === "connected";
  const step = state.account?.status === "connected" || approved ? 2 : 1;
  ["step-usb", "step-pair", "step-device"].forEach((id, index) => {
    $(id).classList.toggle("done", index < step);
    $(id).classList.toggle("current", index === step);
    if (index === step) $(id).setAttribute("aria-current", "step"); else $(id).removeAttribute("aria-current");
  });
  $("ready-status").textContent = state.helper_ready ? state.platform === "macos" ? "USB export preview" : "USB ready" : "Setup needed";
  $("ready-status").classList.toggle("ready", state.helper_ready);
  $("setup").hidden = state.helper_ready;
  $("enable-usb").hidden = !state.setup_available;
  $("enable-usb").disabled = !!state.setup?.running;
  $("enable-usb").textContent = state.setup?.running ? "Preparing USB support…" : "Enable USB sharing";
  const supported = ["linux", "windows", "macos"].includes(state.platform);
  $("platform-badge").textContent = state.platform === "windows" ? "Windows alpha" : state.platform === "linux" ? "Linux alpha" : state.platform === "macos" ? "Mac preview" : "Control preview";
  $("setup-help").href = `https://github.com/cobanov/portrelay/blob/main/docs/${state.platform === "macos" ? "macos" : state.platform === "windows" ? "windows" : "linux"}-alpha.md`;
  $("setup-title").textContent = state.platform === "macos" ? "Install the complete Mac app" : supported ? "Enable USB sharing" : "USB support is coming to macOS";
  $("setup-description").textContent = state.platform === "macos" ? "The Mac app includes its USB export worker. Build or reinstall the complete app to continue. No administrator setup is needed." : !supported
    ? "Sharing or connecting USB devices currently needs Linux or Windows. macOS device support is still in development."
    : state.setup?.running && state.platform === "windows" ? "Approve the Windows permission prompt. If it is hidden, select the flashing shield in the taskbar."
    : state.setup_available ? "Allow PortRelay to prepare USB support. Your system may ask for administrator permission."
    : state.platform === "windows" ? "Install the Windows app to prepare USB support from this window."
    : "Install the Ubuntu or Debian package to complete setup from this window. Manual installations can use the setup guide.";
  $("setup-error").hidden = !state.setup?.error;
  $("setup-error").textContent = state.setup?.error || "";
  $("remote-tab").textContent = state.capabilities?.usb_import === false ? "Other devices" : "Use a remote device";
  $("capability-note").hidden = state.platform !== "macos";
  $("capability-note").textContent = "Mac preview: limited USB export. Receiving USB and sharing Bluetooth are unavailable.";

}
function icon(name, className = "") {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("aria-hidden", "true");
  if (className) svg.setAttribute("class", className);
  const use = document.createElementNS("http://www.w3.org/2000/svg", "use");
  use.setAttribute("href", `#i-${name}`); svg.append(use); return svg;
}
function showScreen(next, focus = false) {
  if (!Object.hasOwn(screens, next)) return;
  screen = next;
  try { sessionStorage.setItem("portrelay-screen", screen); } catch {}
  document.querySelectorAll("[data-panel]").forEach(panel => { panel.hidden = panel.dataset.panel !== screen; });
  document.querySelectorAll("[data-screen]").forEach(item => {
    const active = item.dataset.screen === screen;
    item.classList.toggle("active", active);
    if (active) item.setAttribute("aria-current", "page"); else item.removeAttribute("aria-current");
  });
  $("page-title").textContent = screens[screen][0];
  $("breadcrumb-page").textContent = screens[screen][0];
  $("page-description").textContent = screens[screen][1];
  document.title = `${screens[screen][0]} · PortRelay`;
  $("peer-context").hidden = !["devices", "input"].includes(screen);
  $("account").hidden = !state || (screen !== "settings" && state.account?.status === "connected" && !state.account?.error);
  closeMenu();
  if (focus) $("page-title").focus({ preventScroll: true });
}
function navigate(next) {
  showScreen(next, true);
  if (state && !busy) run(async () => {});
}
function closeMenu() {
  document.body.classList.remove("menu-open");
  $("menu-backdrop").hidden = true;
  $("open-menu").setAttribute("aria-expanded", "false");
}
function renderOverview() {
  const peers = Object.values(state.peers);
  const registered = peers.filter(p => p.approved || p.account_id);
  const online = registered.filter(p => p.account_id && p.approved && p.online).length;
  $("computer-count").textContent = registered.length + 1;
  $("metric-computers").textContent = registered.length + 1;
  $("metric-online").textContent = `${online + 1} online${registered.some(p => !p.account_id) ? " · direct peers checked on use" : ""}`;
  $("metric-shared").textContent = state.devices.filter(d => !d.blocked && state.grants[d.id]?.generation === d.generation && state.grants[d.id].peers.length).length;
  $("metric-connections").textContent = state.sessions.length;
  $("metric-activity").textContent = state.sessions.length ? "Encrypted between your computers" : "Nothing in use right now";
  $("session-count").textContent = state.sessions.length;
  $("session-count").hidden = !state.sessions.length;
  $("computers-caption").textContent = state.account?.identity ? `GitHub · ${state.account.identity.name}` : "This computer and your paired devices";
}
function renderPeers() {
  const entries = Object.entries(state.peers);
  if (!entries.some(([id, p]) => id === selectedPeer && p.approved)) selectedPeer = entries.find(([, p]) => p.approved)?.[0] || "";
  const choices = entries.filter(([, p]) => p.approved).map(([id, p]) => [id, p.name]);
  const picker = $("peer-picker"), signature = JSON.stringify(choices);
  if (picker.dataset.choices !== signature) {
    const none = node("option", choices.length ? "Choose a computer" : "Add a computer first"); none.value = ""; none.disabled = !!choices.length;
    picker.replaceChildren(none, ...choices.map(([id, name]) => { const option = node("option", name); option.value = id; return option; }));
    picker.dataset.choices = signature;
  }
  picker.value = selectedPeer; picker.disabled = !choices.length;
  const container = $("peers");
  const openDetails = new Set([...container.querySelectorAll("details[open]")].map((d) => d.id));
  container.replaceChildren();
  const local = node("div", undefined, "peer-card local-peer");
  const localIcon = node("span", undefined, "peer-icon"); localIcon.append(icon("computers"));
  const localInfo = node("div", undefined, "peer-info"), localTitle = node("h3", state.name);
  localTitle.append(node("span", "This computer", "badge"));
  localInfo.append(localTitle, node("p", `${state.platform === "macos" ? "macOS" : state.platform === "windows" ? "Windows" : "Linux"} · Online`, "muted"));
  const localActions = node("div", undefined, "peer-actions");
  localActions.append(button("Local devices", async () => { setView("local"); showScreen("devices", true); }, "view-local-devices", true));
  local.append(localIcon, localInfo, localActions); container.append(local);
  for (const [id, peer] of entries) {
    const card = node("div", undefined, `peer-card${id === selectedPeer ? " selected" : ""}`);
    const computerIcon = node("span", undefined, "peer-icon"); computerIcon.append(icon("computers"));
    const info = node("div", undefined, "peer-info");
    const title = node("h3", peer.name);
    if (id === selectedPeer) title.append(node("span", "Selected", "badge"));
    info.append(title, node("p", peer.account_id ? (peer.approved ? (peer.online ? "Your account · Online" : "Your account · Offline") : "Account connection paused") : peer.approved ? "Paired directly" : "Wants to connect to you", "muted"));
    const actions = node("div", undefined, "peer-actions");
    if (!peer.approved && !peer.account_id) actions.append(button("Approve", () => action({ op: "approve", peer: id }), `approve-${id}`));
    else if (peer.approved) {
      actions.append(button("USB devices", async () => { selectedPeer = id; setView("remote"); showScreen("devices", true); }, `select-${id}`, true));
      actions.append(button("Control", async () => { selectedPeer = id; showScreen("input", true); }, `control-peer-${id}`));
    }
    card.append(computerIcon, info, actions);
    const details = node("details"); details.id = `manage-${id}`; details.open = openDetails.has(details.id);
    details.append(node("summary", "Manage"));
    const management = node("div", undefined, "peer-management");
    if (peer.approved && state.input?.receive_supported) {
      const allowed = state.input.controllers.includes(id);
      const control = button(allowed ? "Stop allowing control" : "Allow keyboard & mouse", async () => {
        if (!allowed && !await confirmAction(`Allow ${peer.name} to control this Linux desktop with its keyboard and mouse?\n\nThis applies while you are signed in and the desktop is unlocked. You can stop control here at any time.`, "Allow keyboard & mouse?", "Allow control")) return;
        await action({ op: "input_allow", peer: id, allowed: !allowed });
      }, `input-allow-${id}`, true);
      control.disabled = !allowed && !state.input.ready;
      management.append(node("p", allowed ? "Keyboard & mouse control allowed" : "Keyboard & mouse control off", "muted"), control);
      if (!allowed && !state.input.ready) management.append(button("Set up receiving", async () => showScreen("input", true), `setup-input-${id}`, true));
    }
    const identity = node("details"); identity.append(node("summary", "Computer identity"), node("p", id, "fingerprint"));
    management.append(identity, button("Remove computer", async () => {
      if (await confirmAction(`Remove ${peer.name} and close its device connections?`, "Remove computer?", "Remove computer")) await action({ op: "revoke", peer: id });
    }, `remove-${id}`, true));
    details.append(management); card.append(details); container.append(card);
  }
  if (!entries.length) {
    const hint = node("div", undefined, "empty");
    hint.append(node("strong", "Bring your other computer here"), node("p", "Install PortRelay and sign in with the same GitHub account."), button("Add a computer", async () => $("add-dialog").showModal(), "empty-add-computer", true));
    container.append(hint);
  }
  renderOverview();
}
const riskText = {
  input: "Keyboard / mouse: this computer loses access while it is borrowed. Keep another way to control this computer.",
  storage: "Disk: unmount all volumes first (take the disk offline on Windows). Eject it on the receiving computer before disconnecting. A lost connection can lose unsaved data. Do not use this for your only copy of important files.",
  network: "Network adapter: disable it first. This computer cannot use its network connection while it is borrowed.",
  bluetooth: "Bluetooth: use a separate USB adapter. Its local connections stop while borrowed. Keep the peripheral near the original computer, then pair it in Bluetooth settings on the receiving computer."
};
const deviceMetadata = new Map();
function risksFor(device) { return device.risks || (riskText[device.kind] ? [device.kind] : []); }
async function confirmHandoff(devices) {
  const warnings = [...new Set(devices.flatMap(risksFor))].map((r) => riskText[r] || `This device requires acknowledgement: ${r}`);
  return !warnings.length || confirmAction(`${devices.map((d) => d.name).join("\n")}\n\n${warnings.join("\n\n")}\n\nAllow this handoff?`, "Share this device?", "Allow sharing");
}
async function shareDevice(device, peer) {
  return action({ op: "share", device: device.id, generation: device.generation, peer, acknowledge_risks: risksFor(device), acknowledge_bluetooth: risksFor(device).includes("bluetooth") });
}
async function confirmReturn(device) {
  return !device || !risksFor(device).includes("storage") || confirmAction("Eject or unmount every volume on the receiving computer first. Disconnecting while files are in use can lose data. Have you finished and safely ejected this disk?", "Return this disk?", "Ejected, disconnect");
}
function deviceRow(device, protectedDevice = false) {
  const row = node("div", undefined, "device-row"), details = node("div", undefined, "device-details");
  row.append(node("span", device.kind === "bluetooth" ? "ᛒ" : "↔", "device-icon"));
  const grant = state.grants[device.id];
  const shared = grant?.generation === device.generation && grant.peers.includes(selectedPeer);
  const active = state.sessions.find((s) => s.direction === "incoming" && s.device === device.id && s.peer === selectedPeer);
  const key = `${view === "local" ? state.id : selectedPeer}/${device.id}`;
  deviceNames.set(key, device.name); deviceMetadata.set(key, device);
  const status = view === "local" ? shared ? `Shared with ${peerName(selectedPeer)}` : "Private" : active ? labels[active.state] || "Connected" : device.busy ? "In use by another computer" : "Available";
  details.append(node("h3", device.name), node("p", protectedDevice ? device.blocked : status));
  if (!protectedDevice && risksFor(device).length) details.append(node("p", risksFor(device).map((r) => ({ storage: "Disk", input: "Keyboard / mouse", network: "Network adapter", bluetooth: "Whole Bluetooth adapter" })[r] || r).join(" · "), "muted"));
  row.append(details);
  if (protectedDevice) return row;
  if (view === "local") {
    const label = shared ? grant.peers.length > 1 ? "Stop all sharing" : "Stop sharing" : "Share device";
    const b = button(label, async () => {
      if (!selectedPeer) throw new Error("Add and select a computer first.");
      if (shared) { if (await confirmReturn(device)) await action({ op: "unshare", device: device.id }); }
      else if (await confirmHandoff([device])) await shareDevice(device, selectedPeer);
    }, `share-${device.id}`, shared);
    b.disabled = !state.helper_ready || !selectedPeer; row.append(b);
  } else {
    const b = button(active ? "Disconnect" : "Connect", async () => {
      if (active && !await confirmReturn(device)) return;
      tell(active ? "Returning the device to its owner…" : `Connecting ${device.name}…`);
      await action(active ? { op: "disconnect", session: active.id } : { op: "connect", peer: selectedPeer, device: device.id, generation: device.generation });
    }, `connect-${device.id}`, !!active);
    b.disabled = (!active && (device.busy || state.capabilities?.usb_import === false)) || !state.helper_ready || (active && active.state !== "connected");
    if (!active && state.capabilities?.usb_import === false) { b.title = state.capabilities.import_reason; details.append(node("p", "Receiving USB is not available on Mac yet.", "muted")); }
    row.append(b);
  }
  return row;
}
function hubGroup(id, hub, devices) {
  const group = node("section", undefined, "hub-group"), heading = node("div", undefined, "hub-heading");
  const info = node("div"); info.append(node("h3", hub?.name || "USB hub"), node("p", `${devices.length} connected devices · choose individually or share the available devices below`, "muted"));
  const available = devices.filter((d) => !d.blocked);
  const b = button("Share available", async () => {
    const peer = selectedPeer;
    if (!await confirmAction(`Share these devices with ${peerName(peer)}?\n\n${available.map((d) => d.name).join("\n")}\n\nDevices plugged in later stay private.`, "Share this group?", "Continue") || !await confirmHandoff(available)) return;
    const failed = []; let shared = 0;
    for (const device of available) {
      try { await shareDevice(device, peer); shared++; }
      catch (error) { failed.push(`${device.name}: ${error.message}`); }
    }
    tell(`${shared} devices shared.${failed.length ? " " + failed.join("; ") : ""}`, failed.length > 0);
  }, `hub-share-${id}`);
  b.disabled = !available.length || !selectedPeer || !state.helper_ready;
  heading.append(info, b); group.append(heading, ...devices.map((d) => deviceRow(d, !!d.blocked)));
  return group;
}
async function renderDevices(revision) {
  const container = $("device-list");
  $("protected-devices").hidden = true;
  $("device-instruction").textContent = view === "local"
    ? selectedPeer ? `Choose what ${peerName(selectedPeer)} can use. Devices stay here until it connects.` : "Add your other computer, then choose a device to share."
    : state.capabilities?.usb_import === false ? "You can view shared devices here. Receiving USB on a Mac is not available yet." : selectedPeer ? `Devices shared by ${peerName(selectedPeer)}. Select Connect to use one here.` : "Select a computer above to see its devices.";
  let devices = state.devices;
  if (view === "remote") {
    if (!selectedPeer) { empty(container, "No computer selected."); return; }
    try { devices = (await request("/api/action", { op: "remote", peer: selectedPeer })).devices; }
    catch (error) {
      if (revision !== refreshRevision) return;
      empty(container, "Could not load shared devices. Open PortRelay on the other computer and approve this computer there.");
      const details = node("details"); details.append(node("summary", "Connection details"), node("p", error.message)); container.append(details); return;
    }
  }
  if (revision !== refreshRevision) return;
  container.replaceChildren();
  const grouped = new Set();
  if (view === "local") {
    const groups = new Map();
    for (const device of devices.filter((d) => d.parent_hub && d.kind !== "hub")) {
      if (!groups.has(device.parent_hub)) groups.set(device.parent_hub, []);
      groups.get(device.parent_hub).push(device); grouped.add(device.id);
    }
    for (const [id, children] of groups) container.append(hubGroup(id, devices.find((d) => d.id === id), children));
  }
  for (const device of devices.filter((d) => !d.blocked && !grouped.has(d.id))) container.append(deviceRow(device));
  if (!container.children.length) empty(container, view === "local" ? "No shareable device found. Connect a test USB device to this computer." : "No devices shared yet. On the other computer, choose Share from here and select a device.");
  if (view === "local") {
    const protectedDevices = devices.filter((d) => d.blocked && !grouped.has(d.id) && !(d.kind === "hub" && devices.some((child) => child.parent_hub === d.id)));
    $("protected-devices").hidden = !protectedDevices.length;
    $("protected-summary").textContent = `${protectedDevices.length} ${protectedDevices.length === 1 ? "device" : "devices"} kept on this computer`;
    $("protected-list").replaceChildren(...protectedDevices.map((d) => deviceRow(d, true)));
  }
}
function renderSessions() {
  const container = $("sessions"); container.replaceChildren();
  const errors = state.history.slice(0, 3).filter((s) => s.error);
  $("connections-section").hidden = false;
  if (!state.sessions.length && !errors.length) {
    const hint = node("div", undefined, "empty");
    hint.append(node("strong", "No active connections"), node("p", "Devices stay on their own computers until you connect."), button("Browse USB devices", async () => showScreen("devices", true), "browse-devices", true));
    container.append(hint);
  }
  for (const session of state.sessions) {
    const key = `${session.direction === "outgoing" ? state.id : session.peer}/${session.device}`;
    const metadata = session.metadata || deviceMetadata.get(key);
    const inputSession = session.direction.startsWith("input-");
    const name = inputSession ? "Keyboard & mouse" : metadata?.name || deviceNames.get(key) || "USB device";
    const row = node("div", undefined, "session"), info = node("div");
    info.append(node("strong", name), node("p", `${labels[session.state] || session.state} · ${session.direction === "input-sending" ? "Controlling" : session.direction === "outgoing" ? "Shared with" : "From"} ${peerName(session.peer)}`));
    const b = button("Disconnect", async () => { if (await confirmReturn(metadata)) await action({ op: "disconnect", session: session.id }); }, `disconnect-${session.id}`, true);
    b.disabled = ["restoring", "detaching"].includes(session.state);
    row.append(info, b); container.append(row);
    if (metadata && risksFor(metadata).includes("bluetooth") && session.direction === "incoming" && session.state === "connected") {
      const guide = node("div", undefined, "bluetooth-guide");
      guide.append(node("h3", "Pair a Bluetooth device"), node("p", `Keep it near ${peerName(session.peer)}, where the adapter is plugged in. Put it in pairing mode, then select the borrowed adapter in this computer's Bluetooth settings. Physical adapter compatibility is still experimental.`), button("Open Bluetooth settings", () => action({ op: "bluetooth_settings" }), `bluetooth-${session.id}`, true));
      container.append(guide);
    }
  }
  for (const session of errors) {
    const details = node("details", undefined, "notice");
    details.append(node("summary", "A connection needs attention"), node("p", session.error)); container.append(details);
  }
}
async function refresh() {
  const revision = ++refreshRevision;
  const focused = document.activeElement?.id;
  const nextState = await request("/api/state");
  if (revision !== refreshRevision) return;
  state = nextState;
  $("computer-name").textContent = state.name;
  $("network").textContent = state.network.includes("Relay only") ? "Internet relay · encrypted" : state.network.includes("relay") ? "Direct or internet relay · encrypted" : "Local network · encrypted";
  $("version").textContent = `PortRelay ${state.version}`;
  if (document.activeElement !== $("name-input") && !$("name-input").dataset.edited) $("name-input").value = state.name;
  $("identity").textContent = `Computer identity: ${state.id}`;
  $("helper-details").textContent = state.helper_ready ? "USB service ready." : state.helper_error || "USB service not ready.";
  renderAccount(); renderSetup(); renderPeers(); renderInput();
  if (!screenInitialized) { showScreen(screen); screenInitialized = true; }
  if (screen === "devices") await renderDevices(revision);
  if (revision !== refreshRevision) return;
  renderSessions();
  if (focused && (!document.activeElement?.id || document.activeElement.id === focused)) $(focused)?.focus({ preventScroll: true });
}
async function invitation() {
  if (!currentInvitation || Date.now() >= invitationExpiry * 1000) {
    const result = await action({ op: "invite" });
    currentInvitation = result.invitation; invitationExpiry = result.expires;
    $("invitation-out").value = currentInvitation;
  }
  return currentInvitation;
}
$("enable-usb").onclick = () => run(() => action({ op: "setup_usb" }), $("enable-usb"));
// Keep the URL fragment reserved for the local authentication handoff.
document.querySelector(".skip-link").onclick = event => {
  event.preventDefault(); $("workspace").focus();
};
$("add-computer").onclick = () => $("add-dialog").showModal();
$("close-add").onclick = () => $("add-dialog").close();
$("add-account").onclick = () => { $("add-dialog").close(); navigate("settings"); };
$("peer-picker").onchange = () => run(async () => { selectedPeer = $("peer-picker").value; });
for (const item of document.querySelectorAll("[data-screen], [data-go]")) item.onclick = () => navigate(item.dataset.screen || item.dataset.go);
$("open-menu").onclick = () => {
  document.body.classList.add("menu-open"); $("menu-backdrop").hidden = false;
  $("open-menu").setAttribute("aria-expanded", "true");
  $("close-menu").focus();
};
$("close-menu").onclick = $("menu-backdrop").onclick = () => { closeMenu(); $("open-menu").focus(); };
document.addEventListener("keydown", event => {
  if (!document.body.classList.contains("menu-open")) return;
  if (event.key === "Escape") { event.preventDefault(); closeMenu(); $("open-menu").focus(); }
  if (event.key === "Tab") {
    const items = [...$("sidebar").querySelectorAll("a, button")].filter(el => el.getClientRects().length);
    const first = items[0], last = items.at(-1);
    if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last.focus(); }
    else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first.focus(); }
  }
});
$("create-mode").onclick = () => setPairMode(true);
$("join-mode").onclick = () => setPairMode(false);
$("copy-invitation").onclick = () => run(async () => {
  const value = await invitation();
  try { await navigator.clipboard.writeText(value); $("invite-status").textContent = "Copied. Paste it into PortRelay on your other computer, then approve its request here."; }
  catch { $("invitation-details").open = true; $("invitation-out").select(); tell("Select and copy the invitation below, or save it as a file."); }
}, $("copy-invitation"));
$("save-invitation").onclick = () => run(async () => {
  const url = URL.createObjectURL(new Blob([await invitation()], { type: "text/plain" }));
  const a = node("a"); a.href = url; a.download = "Pair-with-PortRelay.portrelay"; a.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
  $("invite-status").textContent = "Open this file in PortRelay on the other computer within 10 minutes. Keep it private.";
}, $("save-invitation"));
$("invitation-file").onchange = () => run(async () => {
  const file = $("invitation-file").files[0]; if (!file) return;
  if (file.size > 8192) throw new Error("This invitation file is too large. Choose a PortRelay invitation.");
  $("invitation-in").value = await file.text();
  tell("Invitation loaded. Choose Add computer to continue.");
});
$("pair").onclick = () => run(async () => {
  await action({ op: "pair", invitation: $("invitation-in").value });
  $("invitation-in").value = ""; setView("remote"); showScreen("computers", true);
}, $("pair"));
$("name-input").oninput = () => { $("name-input").dataset.edited = "true"; };
$("save-name").onclick = () => run(async () => {
  await action({ op: "rename", name: $("name-input").value });
  delete $("name-input").dataset.edited; tell("Computer name saved.");
}, $("save-name"));
$("refresh").onclick = () => run(async () => {}, $("refresh"));
for (const next of ["local", "remote"]) $(next + "-tab").onclick = () => run(async () => setView(next));
refresh().then(() => tell("")).catch((error) => tell(error.message, true));
setInterval(() => {
  if (busy || backgroundRefreshing || document.visibilityState !== "visible") return;
  backgroundRefreshing = true;
  refresh().catch((error) => { if (!busy) tell(error.message, true); }).finally(() => { backgroundRefreshing = false; });
}, 3000);

function renderAccount() {
  const account = state.account || {status:"signed_out"};
  const connected = !!account.identity, pending = account.status === "pending";
  $("account").hidden = screen !== "settings" && account.status === "connected" && !account.error;
  $("sidebar-account").textContent = connected ? account.identity.name : "Your account";
  $("sidebar-account-status").textContent = connected ? account.status === "offline" ? "Connection paused" : "GitHub account" : pending ? "Finish sign-in" : "Sign in to connect";
  $("account-avatar").textContent = connected ? account.identity.name.slice(0, 2).toUpperCase() : "P";
  $("add-account-hint").textContent = connected ? `Use your ${account.identity.name} GitHub account.` : "Use the same account on both computers.";
  $("account-title").textContent = connected ? `Signed in as ${account.identity.name}` : pending ? "Finish sign-in in your browser" : "Your computers. One sign-in.";
  $("account-description").textContent = connected ? account.status === "offline" ? "Account service unavailable. Device access pauses until membership can be checked." : "Install PortRelay and sign in with this GitHub account on your other computers." : pending ? "Approve this computer on the GitHub sign-in page. This window updates automatically." : "Sign in with the same GitHub account on each computer. They appear here automatically.";
  $("sign-in").hidden = connected || pending;
  $("sign-out").hidden = !connected && !pending;
  $("sign-out").textContent = pending ? "Cancel sign-in" : "Sign out";
  $("continue-login").hidden = !pending;
  if (pending) $("continue-login").href = account.authorization_url;
  $("account-error").hidden = !account.error;
  $("account-error").textContent = account.error || "";
}
$("sign-in").onclick = () => {
  if (busy) return;
  const popup = window.open("about:blank", "_blank");
  if (popup) popup.opener = null;
  run(async () => {
    try {const result = await action({op:"account_login"}); if (popup) popup.location.replace(result.authorization_url);}
    catch (error) {popup?.close(); throw error;}
  }, $("sign-in"));
};
$("sign-out").onclick = () => run(async () => {
  if (state.account?.identity && !await confirmAction("Its account connections and sharing permissions will close. Eject any borrowed disks first.", "Sign out this computer?", "Sign out")) return;
  await action({op:"account_logout"});
}, $("sign-out"));
