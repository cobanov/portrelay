// This is an in-memory interface demo. It never requests USB, Bluetooth,
// network discovery, device permissions, or a connection to a local agent.
const peers = {
  studio: {
    name: "Studio PC",
    network: "Local network",
    devices: [
      {
        id: "studio-serial",
        name: "USB Serial Adapter",
        type: "USB",
        icon: "usb",
        connected: false,
      },
      {
        id: "studio-bluetooth",
        name: "Bluetooth Adapter",
        type: "BT",
        icon: "bluetooth",
        connected: false,
      },
      {
        id: "studio-webcam",
        name: "USB Webcam",
        type: "USB",
        icon: "camera",
        busy: "Design desktop",
      },
    ],
  },
  home: {
    name: "Home server",
    network: "Internet · Relay",
    devices: [
      {
        id: "home-serial",
        name: "USB Serial Adapter",
        type: "USB",
        icon: "usb",
        connected: false,
      },
      {
        id: "home-bluetooth",
        name: "Bluetooth Adapter",
        type: "BT",
        icon: "bluetooth",
        connected: false,
      },
    ],
  },
};
const localDevices = [
  {
    id: "local-serial",
    name: "USB Serial Adapter",
    type: "USB",
    icon: "usb",
    shared: false,
  },
  {
    id: "local-bluetooth",
    name: "Bluetooth Adapter",
    type: "BT",
    icon: "bluetooth",
    shared: false,
  },
  {
    id: "local-keyboard",
    name: "Built-in Keyboard",
    type: "USB",
    icon: "laptop",
    protected: true,
  },
];
const platforms = {
  linux: {
    stage: "FIRST PLATFORM PLANNED",
    title: "Linux goes first.",
    description:
      "We’re starting with USB sharing between Linux computers, then testing dedicated Bluetooth adapters.",
  },
  windows: {
    stage: "NEXT PLATFORM PLANNED",
    title: "Windows is next.",
    description:
      "USB sharing on Windows is planned after Linux. Both sending and receiving devices need their own driver and compatibility checks.",
  },
  macos: {
    stage: "UNDER INVESTIGATION",
    title: "macOS needs a closer look.",
    description:
      "USB support depends on Apple’s device permissions. There is no confirmed macOS USB release yet. Individual BLE support is a separate future goal.",
  },
};

let view = "remote";
let activePeer = "studio";
const byId = (id) => document.getElementById(id);
const svgNS = "http://www.w3.org/2000/svg";

function icon(name) {
  const svg = document.createElementNS(svgNS, "svg");
  svg.setAttribute("class", "icon");
  svg.setAttribute("aria-hidden", "true");
  const use = document.createElementNS(svgNS, "use");
  use.setAttribute("href", `#i-${name}`);
  svg.append(use);
  return svg;
}

function element(tag, className, text) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}

function announce(message) {
  byId("demo-announcement").textContent = message;
}

function updateHint(message) {
  byId("app-hint").textContent = message;
  announce(message);
}

function renderDevices() {
  const focusedId = document.activeElement?.id;
  const local = view === "local";
  const peer = peers[activePeer];
  const devices = local ? localDevices : peer.devices;
  byId("app-breadcrumb").textContent = local
    ? "This computer /"
    : "Other computers /";
  byId("computer-name").textContent = local ? "Work laptop" : peer.name;
  const badge = byId("network-badge");
  badge.replaceChildren(
    element("span", "status-dot"),
    document.createTextNode(local ? "Local devices" : peer.network),
  );
  byId("device-count").replaceChildren(
    document.createTextNode(local ? "Your devices " : "Shared devices "),
    element("span", "mono", String(devices.length)),
  );
  const list = byId("device-list");
  list.replaceChildren();
  for (const device of devices) {
    const active = local ? device.shared : device.connected;
    const row = element("div", `device-row${active ? " connected" : ""}`);
    const deviceIcon = element("span", "device-icon");
    deviceIcon.append(icon(device.icon));
    const details = element("div", "device-details");
    details.append(element("h3", "", device.name));
    let status;
    if (local) {
      status = device.protected
        ? "Kept on this computer"
        : device.shared
          ? "Shared in preview"
          : "Private to this computer";
    } else {
      status = device.busy
        ? `Used by ${device.busy}`
        : device.connected
          ? "Connected in preview"
          : device.type === "BT"
            ? "Share the whole adapter"
            : "Available to connect";
    }
    const caption = element("p");
    caption.append(
      element("span", "device-type", device.type),
      element("span", "", status),
    );
    details.append(caption);
    const action = device.protected
      ? "Local only"
      : device.busy
        ? "In use"
        : local
          ? device.shared
            ? "Stop sharing"
            : "Share"
          : device.connected
            ? "Disconnect"
            : "Connect";
    const button = element("button", "device-action", action);
    button.type = "button";
    button.id = `action-${device.id}`;
    button.dataset.device = device.id;
    button.setAttribute("aria-label", `${action}: ${device.name} in preview`);
    button.disabled = Boolean(device.busy || device.protected);
    row.append(deviceIcon, details, button);
    list.append(row);
  }
  for (const button of document.querySelectorAll("[data-view]")) {
    const selected = button.dataset.view === view;
    button.classList.toggle("selected", selected);
    button.setAttribute("aria-pressed", String(selected));
  }
  for (const button of document.querySelectorAll("[data-peer]")) {
    const selected = !local && button.dataset.peer === activePeer;
    button.classList.toggle("selected", selected);
    button.setAttribute("aria-pressed", String(selected));
  }
  const connected = Object.values(peers)
    .flatMap((item) => item.devices)
    .filter((device) => device.connected).length;
  byId("session-status").textContent = `${connected} connected`;
  if (focusedId) byId(focusedId)?.focus({ preventScroll: true });
}

byId("device-list").addEventListener("click", (event) => {
  const button = event.target.closest("button[data-device]");
  if (!button || button.disabled) return;
  const local = view === "local";
  const devices = local ? localDevices : peers[activePeer].devices;
  const device = devices.find((item) => item.id === button.dataset.device);
  if (!device || device.busy || device.protected) return;
  if (local) {
    device.shared = !device.shared;
    renderDevices();
    updateHint(
      device.shared
        ? `${device.name} is shared with paired computers in this preview.${device.type === "BT" ? " This shares the whole adapter." : ""}`
        : `${device.name} is private again in this preview.`,
    );
  } else {
    device.connected = !device.connected;
    renderDevices();
    updateHint(
      device.connected
        ? device.type === "BT"
          ? "The whole Bluetooth adapter is assigned to Work laptop in this preview. Nearby devices would pair on Work laptop."
          : `${device.name} is connected in this preview. No physical device is accessed.`
        : `${device.name} is available on ${peers[activePeer].name} again in this preview.`,
    );
  }
});

for (const button of document.querySelectorAll("[data-view]")) {
  button.disabled = false;
  button.addEventListener("click", () => {
    if (!["local", "remote"].includes(button.dataset.view)) return;
    view = button.dataset.view;
    renderDevices();
    updateHint(
      view === "local"
        ? "Your devices stay private until you choose to share them. Try a Share button."
        : "Choose a device to try the connection flow.",
    );
  });
}

for (const button of document.querySelectorAll("[data-peer]")) {
  button.disabled = false;
  button.addEventListener("click", () => {
    if (!Object.hasOwn(peers, button.dataset.peer)) return;
    activePeer = button.dataset.peer;
    view = "remote";
    renderDevices();
    updateHint(
      activePeer === "home"
        ? "An example internet connection using a relay. The devices and connection are simulated."
        : "Choose a device to try the connection flow.",
    );
  });
}

for (const button of document.querySelectorAll("[data-os]")) {
  button.disabled = false;
  button.addEventListener("click", () => {
    if (!Object.hasOwn(platforms, button.dataset.os)) return;
    const platform = platforms[button.dataset.os];
    for (const option of document.querySelectorAll("[data-os]")) {
      const selected = option === button;
      option.classList.toggle("selected", selected);
      option.setAttribute("aria-pressed", String(selected));
    }
    byId("platform-stage").textContent = platform.stage;
    byId("platform-title").textContent = platform.title;
    byId("platform-description").textContent = platform.description;
  });
}

renderDevices();
