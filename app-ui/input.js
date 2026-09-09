let control = null, controlEpoch = 0, requestedPeer = "";
const pad = $("control-pad");
function renderInput() {
  $("control-start").textContent = selectedPeer ? `Control ${peerName(selectedPeer)}` : "Add a computer first";
  $("control-start").disabled = !selectedPeer || !!control || !!requestedPeer;
  $("input-description").textContent = selectedPeer ? `Use this keyboard and mouse on ${peerName(selectedPeer)}. Allow control in PortRelay on that Linux desktop first.` : "Add a Linux computer, then use your keyboard and mouse to control it.";
  $("enable-input").hidden = !state.input?.receive_supported || state.input.ready || !state.input.setup_available;
  $("enable-input").disabled = !!state.input?.setup?.running;
  $("input-setup-status").textContent = state.input?.setup?.running ? "Approve the system permission window…" : state.input?.setup?.error || (state.input?.receive_supported ? state.input.ready ? "This desktop can receive control. Choose which computer to allow above." : "Receiving needs one-time setup and an unlocked desktop. Terminal: sudo /usr/lib/portrelay/setup-input" : "Send control to Linux. Receiving on Mac and Windows is still in development.");
  const active = state.sessions.find(s => s.direction === "input-receiving");
  $("input-badge").textContent = active ? `Controlled by ${peerName(active.peer)}` : "Linux receiver";
}
function disconnectControl(session) {
  // Keepalive covers tab closing; the receiver also expires missing heartbeats.
  return fetch("/api/action", { method: "POST", keepalive: true, headers: { Authorization: `Bearer ${token}`, "Content-Type": "application/json" }, body: JSON.stringify({ op: "disconnect", session }) }).catch(() => {});
}
function stopControl(message = "Control ended. Your keyboard and mouse are back here.", error = false) {
  controlEpoch++;
  const previous = control;
  control = null; requestedPeer = "";
  if (previous) { clearTimeout(previous.timer); disconnectControl(previous.session); }
  if (document.pointerLockElement === pad) document.exitPointerLock();
  pad.hidden = true;
  if (state) renderInput();
  tell(message, error);
}
async function beginControl() {
  const peer = requestedPeer, epoch = controlEpoch;
  if (!peer || control) return;
  $("control-title").textContent = `Connecting to ${peerName(peer)}…`;
  try {
    const result = await request("/api/action", { op: "input_start", peer });
    if (epoch !== controlEpoch || document.pointerLockElement !== pad || document.hidden || !document.hasFocus()) { disconnectControl(result.session); return; }
    requestedPeer = "";
    control = { session: result.session, sequence: 1, queue: new InputEvents.Queue(), timer: null, last: 0 };
    $("control-title").textContent = `Controlling ${peerName(peer)}`;
    tell(`Controlling ${peerName(peer)}. Press Esc to return here.`);
    renderInput(); pumpControl(control);
  } catch (error) { if (epoch === controlEpoch) stopControl(error.message, true); }
}
async function pumpControl(current) {
  if (control !== current) return;
  if (document.hidden || !document.hasFocus() || document.pointerLockElement !== pad) { stopControl(); return; }
  try {
    if (current.queue.events.length || performance.now() - current.last >= 200) {
      await request("/api/action", { op: "input_events", session: current.session, batch: { sequence: current.sequence++, events: current.queue.take() } });
      current.last = performance.now();
    }
    if (control === current) current.timer = setTimeout(() => pumpControl(current), 16);
  } catch (error) { if (control === current) stopControl(error.message, true); }
}
function capture(event, task) {
  if (document.pointerLockElement !== pad) return;
  event.preventDefault(); event.stopPropagation();
  if (!control) return;
  try { task(control.queue); } catch (error) { stopControl(error.message, true); }
}
$("control-start").onclick = () => {
  if (!selectedPeer || control || requestedPeer) return;
  if (!pad.requestPointerLock) { tell("Use a desktop browser with pointer lock support, such as Chrome.", true); return; }
  requestedPeer = selectedPeer; controlEpoch++;
  const epoch = controlEpoch;
  pad.hidden = false; pad.focus(); renderInput();
  $("control-title").textContent = "Allow mouse control in your browser…";
  // Request inside the user's click, before any network await consumes activation.
  try { const pending = pad.requestPointerLock(); pending?.catch(error => { if (controlEpoch === epoch) stopControl(`Mouse control could not start: ${error.message} Keep this tab in front, then click Control again.`, true); }); }
  catch (error) { stopControl(error.message, true); }
};
$("enable-input").onclick = () => run(() => action({ op: "setup_input" }), $("enable-input"));
document.addEventListener("pointerlockchange", () => {
  if (document.pointerLockElement === pad) { beginControl(); }
  else if (control || requestedPeer) stopControl();
});
document.addEventListener("pointerlockerror", () => { if (requestedPeer) stopControl("Keep the PortRelay tab in front, then click Control again and allow mouse control if your browser asks.", true); });
window.addEventListener("blur", () => { if (control || requestedPeer) stopControl(); });
window.addEventListener("pagehide", () => { if (control || requestedPeer) stopControl(); });
document.addEventListener("visibilitychange", () => { if (document.hidden && (control || requestedPeer)) stopControl(); });
document.addEventListener("mousemove", event => capture(event, queue => queue.move(event.movementX, event.movementY)), true);
document.addEventListener("wheel", event => capture(event, queue => queue.wheel(event.deltaX, event.deltaY, event.deltaMode)), { capture: true, passive: false });
for (const type of ["mousedown", "mouseup"]) document.addEventListener(type, event => capture(event, queue => {
  if (event.button <= 4) queue.push({ type: "button", button: event.button, down: type === "mousedown" });
}), true);
document.addEventListener("contextmenu", event => capture(event, () => {}), true);
for (const type of ["keydown", "keyup"]) document.addEventListener(type, event => {
  if (event.code === "Escape" && (control || requestedPeer)) { event.preventDefault(); stopControl(); return; }
  capture(event, queue => {
    const code = InputEvents.keys[event.code];
    if (code && !event.isComposing) queue.push({ type: "key", code, value: type === "keyup" ? 0 : event.repeat ? 2 : 1 });
  });
}, true);
