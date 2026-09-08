const $ = (id) => document.getElementById(id);
const token = location.hash.slice(1) || sessionStorage.getItem("portrelay-token") || "";
if (location.hash) {
  sessionStorage.setItem("portrelay-token", token);
  history.replaceState(null, "", "/");
}
let state, selectedPeer = "", view = "local", busy = false;
let refreshRevision = 0, backgroundRefreshing = false;
let renameOpen = false, pairingOpen = false, pairingDismissed = false, currentInvitation = "", invitationExpiry = 0;
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
async function request(path, data) {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), data?.op === "connect" ? 60000 : 25000);
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
  $("name-setup").hidden = approved && !renameOpen;
  document.querySelector(".computers").hidden = !Object.keys(state.peers).length;
  document.querySelector(".devices").hidden = !approved;
  const step = !state.helper_ready ? 0 : !approved ? 1 : 2;
  ["step-usb", "step-pair", "step-device"].forEach((id, index) => {
    $(id).classList.toggle("done", index < step);
    $(id).classList.toggle("current", index === step);
    if (index === step) $(id).setAttribute("aria-current", "step"); else $(id).removeAttribute("aria-current");
  });
  $("ready-status").textContent = state.helper_ready ? "USB ready" : "Setup needed";
  $("ready-status").classList.toggle("ready", state.helper_ready);
  $("setup").hidden = state.helper_ready;
  $("enable-usb").hidden = !state.setup_available;
  $("enable-usb").disabled = !!state.setup?.running;
  $("enable-usb").textContent = state.setup?.running ? "Preparing USB support…" : "Enable USB sharing";
  const supported = ["linux", "windows"].includes(state.platform);
  $("platform-badge").textContent = state.platform === "windows" ? "Windows alpha" : state.platform === "linux" ? "Linux alpha" : "Control preview";
  $("setup-help").href = `https://github.com/cobanov/portrelay/blob/main/docs/${state.platform === "windows" ? "windows" : "linux"}-alpha.md`;
  $("setup-title").textContent = supported ? "Enable USB sharing" : "USB support is coming to macOS";
  $("setup-description").textContent = !supported
    ? "Sharing or connecting USB devices currently needs Linux or Windows. macOS device support is still in development."
    : state.setup?.running && state.platform === "windows" ? "Approve the Windows permission prompt. If it is hidden, select the flashing shield in the taskbar."
    : state.setup_available ? "Allow PortRelay to prepare USB support. Your system may ask for administrator permission."
    : state.platform === "windows" ? "Install the Windows app to prepare USB support from this window."
    : "Install the Ubuntu or Debian package to complete setup from this window. Manual installations can use the setup guide.";
  $("setup-error").hidden = !state.setup?.error;
  $("setup-error").textContent = state.setup?.error || "";
  $("pairing").hidden = !(pairingOpen || (!approved && state.helper_ready && !pairingDismissed));
}
function renderPeers() {
  const entries = Object.entries(state.peers);
  if (!entries.some(([id, p]) => id === selectedPeer && p.approved)) selectedPeer = entries.find(([, p]) => p.approved)?.[0] || "";
  const container = $("peers");
  const openDetails = new Set([...container.querySelectorAll("details[open]")].map((d) => d.id));
  container.replaceChildren();
  for (const [id, peer] of entries) {
    const card = node("div", undefined, `peer-card${id === selectedPeer ? " selected" : ""}`);
    card.append(node("h3", peer.name), node("p", peer.approved ? "Added computer" : "Wants to connect to you", "muted"));
    if (!peer.approved) card.append(button("Approve computer", () => action({ op: "approve", peer: id }), `approve-${id}`));
    else card.append(button(id === selectedPeer ? "Selected" : "Select computer", async () => { selectedPeer = id; }, `select-${id}`, true));
    const details = node("details"); details.id = `manage-${id}`; details.open = openDetails.has(details.id);
    details.append(node("summary", "Manage computer"), node("p", id, "fingerprint"));
    details.append(button("Remove computer", async () => {
      if (confirm(`Remove ${peer.name} and close its device connections?`)) await action({ op: "revoke", peer: id });
    }, `remove-${id}`, true));
    card.append(details); container.append(card);
  }
  if (!entries.length) empty(container, "Add your other computer to see its shared devices.");
}
const riskText = {
  input: "Keyboard / mouse: this computer loses access while it is borrowed. Keep another way to control this computer.",
  storage: "Disk: unmount all volumes first (take the disk offline on Windows). Eject it on the receiving computer before disconnecting. A lost connection can lose unsaved data. Do not use this for your only copy of important files.",
  network: "Network adapter: disable it first. This computer cannot use its network connection while it is borrowed.",
  bluetooth: "Bluetooth: use a separate USB adapter. Its local connections stop while borrowed. Keep the peripheral near the original computer, then pair it in Bluetooth settings on the receiving computer."
};
const deviceMetadata = new Map();
function risksFor(device) { return device.risks || (riskText[device.kind] ? [device.kind] : []); }
function confirmHandoff(devices) {
  const warnings = [...new Set(devices.flatMap(risksFor))].map((r) => riskText[r] || `This device requires acknowledgement: ${r}`);
  return !warnings.length || confirm(`${devices.map((d) => d.name).join("\n")}\n\n${warnings.join("\n\n")}\n\nAllow this handoff?`);
}
async function shareDevice(device, peer) {
  return action({ op: "share", device: device.id, peer, acknowledge_risks: risksFor(device), acknowledge_bluetooth: risksFor(device).includes("bluetooth") });
}
function confirmReturn(device) {
  return !device || !risksFor(device).includes("storage") || confirm("Eject or unmount every volume on the receiving computer first. Disconnecting while files are in use can lose data. Have you finished and safely ejected this disk?");
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
      if (shared) { if (confirmReturn(device)) await action({ op: "unshare", device: device.id }); }
      else if (confirmHandoff([device])) await shareDevice(device, selectedPeer);
    }, `share-${device.id}`, shared);
    b.disabled = !state.helper_ready || !selectedPeer; row.append(b);
  } else {
    const b = button(active ? "Disconnect" : "Connect", async () => {
      if (active && !confirmReturn(device)) return;
      tell(active ? "Returning the device to its owner…" : `Connecting ${device.name}…`);
      await action(active ? { op: "disconnect", session: active.id } : { op: "connect", peer: selectedPeer, device: device.id, generation: device.generation });
    }, `connect-${device.id}`, !!active);
    b.disabled = (!active && device.busy) || !state.helper_ready || (active && active.state !== "connected"); row.append(b);
  }
  return row;
}
function hubGroup(id, hub, devices) {
  const group = node("section", undefined, "hub-group"), heading = node("div", undefined, "hub-heading");
  const info = node("div"); info.append(node("h3", hub?.name || "USB hub"), node("p", `${devices.length} connected devices · choose individually or share the available devices below`, "muted"));
  const available = devices.filter((d) => !d.blocked);
  const b = button("Share available", async () => {
    const peer = selectedPeer;
    if (!confirm(`Share these devices with ${peerName(peer)}?\n\n${available.map((d) => d.name).join("\n")}\n\nDevices plugged in later stay private.`) || !confirmHandoff(available)) return;
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
    : selectedPeer ? `Devices shared by ${peerName(selectedPeer)}. Select Connect to use one here.` : "Select a computer above to see its devices.";
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
    $("protected-summary").textContent = `${protectedDevices.length} devices kept on this computer`;
    $("protected-list").replaceChildren(...protectedDevices.map((d) => deviceRow(d, true)));
  }
}
function renderSessions() {
  const container = $("sessions"); container.replaceChildren();
  const errors = state.history.slice(0, 3).filter((s) => s.error);
  $("connections-section").hidden = !state.sessions.length && !errors.length;
  for (const session of state.sessions) {
    const key = `${session.direction === "outgoing" ? state.id : session.peer}/${session.device}`;
    const metadata = session.metadata || deviceMetadata.get(key);
    const name = metadata?.name || deviceNames.get(key) || "USB device";
    const row = node("div", undefined, "session"), info = node("div");
    info.append(node("strong", name), node("p", `${labels[session.state] || session.state} · ${session.direction === "outgoing" ? "Shared with" : "From"} ${peerName(session.peer)}`));
    const b = button("Disconnect", async () => { if (confirmReturn(metadata)) await action({ op: "disconnect", session: session.id }); }, `disconnect-${session.id}`, true);
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
    details.append(node("summary", "A device connection needs attention"), node("p", session.error)); container.append(details);
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
  renderSetup(); renderPeers(); await renderDevices(revision);
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
$("add-computer").onclick = () => { pairingOpen = true; $("pairing").hidden = false; $("pairing").scrollIntoView({ block: "nearest" }); };
$("close-pairing").onclick = () => { pairingOpen = false; pairingDismissed = true; $("pairing").hidden = true; };
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
  $("invitation-in").value = ""; pairingOpen = false; pairingDismissed = true; setView("remote");
}, $("pair"));
$("change-name").onclick = () => { renameOpen = true; $("name-setup").hidden = false; $("name-input").focus(); };
$("name-input").oninput = () => { $("name-input").dataset.edited = "true"; };
$("save-name").onclick = () => run(async () => {
  await action({ op: "rename", name: $("name-input").value });
  delete $("name-input").dataset.edited; renameOpen = false;
}, $("save-name"));
$("refresh").onclick = () => run(async () => {}, $("refresh"));
for (const next of ["local", "remote"]) $(next + "-tab").onclick = () => run(async () => setView(next));
refresh().then(() => tell("Choose a device when both computers are ready.")).catch((error) => tell(error.message, true));
setInterval(() => {
  if (busy || backgroundRefreshing || document.visibilityState !== "visible") return;
  backgroundRefreshing = true;
  refresh().catch((error) => { if (!busy) tell(error.message, true); }).finally(() => { backgroundRefreshing = false; });
}, 3000);
