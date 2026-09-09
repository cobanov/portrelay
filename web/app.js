// This is an in-memory interface demo. It never requests USB, Bluetooth,
// network discovery, device permissions, or a connection to a local agent.
const peers = {
  studio: {
    name: "Studio PC",
    network: "Local network",
    devices: [
      {
        id: "studio-printer",
        name: "Desk printer",
        type: "USB",
        icon: "printer",
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
        id: "home-drive",
        name: "USB drive",
        type: "USB",
        icon: "drive",
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
    stage: "ALPHA.6 · EXPERIMENTAL",
    title: "Linux, including Raspberry Pi.",
    description: "Ubuntu, Debian or Raspberry Pi OS. Use the app window or a simple SSH menu.",
  },
  windows: {
    stage: "ALPHA.6 · EXPERIMENTAL",
    title: "Windows 11, one installer.",
    description: "Share USB with Linux. Tested both ways with virtual serial devices.",
  },
  macos: {
    stage: "UNDER INVESTIGATION",
    title: "Exploring macOS.",
    description: "Sharing select devices from a Mac comes first. Using remote USB on a Mac has a separate Apple permission gate. Neither is available yet.",
  },
};
const examples = {
  printer: { peer: "studio", device: "studio-printer" },
  bluetooth: { peer: "studio", device: "studio-bluetooth" },
  drive: { peer: "home", device: "home-drive" },
};
let selectedDeviceId = "studio-printer";

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
  if (!devices.some((device) => device.id === selectedDeviceId))
    selectedDeviceId = devices[0].id;
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
    const row = element(
      "div",
      `device-row${active ? " connected" : ""}${device.id === selectedDeviceId ? " highlighted" : ""}`,
    );
    const deviceIcon = element("span", "device-icon");
    deviceIcon.append(icon(device.icon));
    const details = element("div", "device-details");
    details.append(element("h3", "", device.name));
    let status;
    if (local) {
      status = device.protected
        ? "Protected device"
        : device.shared
          ? "Shared in demo"
          : "Private";
    } else {
      status = device.busy
        ? `Used by ${device.busy}`
        : device.connected
          ? "Connected"
          : device.type === "BT"
            ? "Whole adapter"
            : "Available";
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
  renderFlow(devices.find((device) => device.id === selectedDeviceId));
  if (focusedId) byId(focusedId)?.focus({ preventScroll: true });
}

byId("device-list").addEventListener("click", (event) => {
  const button = event.target.closest("button[data-device]");
  if (!button || button.disabled) return;
  const local = view === "local";
  const devices = local ? localDevices : peers[activePeer].devices;
  const device = devices.find((item) => item.id === button.dataset.device);
  if (!device || device.busy || device.protected) return;
  selectedDeviceId = device.id;
  if (local) {
    device.shared = !device.shared;
    renderDevices();
    updateHint(
      device.shared
        ? `${device.name} is shared in this demo.${device.type === "BT" ? " The whole adapter is shared." : ""}`
        : `${device.name} is private again in this demo.`,
    );
  } else {
    device.connected = !device.connected;
    renderDevices();
    updateHint(
      device.connected
        ? device.type === "BT"
          ? `Adapter connected in demo. Pair devices near ${peers[activePeer].name} from Work laptop.`
          : `${device.name} connected in this demo.${device.icon === "printer" ? " Ready to print." : device.icon === "drive" ? " Ready to browse." : ""}`
        : `${device.name} returned to ${peers[activePeer].name} in this demo.`,
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
        ? "Private until you choose to share. Try a Share button."
        : "Choose a device and click Connect.",
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
        ? "An example connection across the internet."
        : "Choose a device and click Connect.",
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
    byId("linux-downloads").hidden = button.dataset.os !== "linux";
    byId("windows-downloads").hidden = button.dataset.os !== "windows";
    byId("windows-limit").hidden = button.dataset.os !== "windows";
    const linux = button.dataset.os === "linux";
    byId("command-install").hidden = button.dataset.os === "macos";
    byId("command-label").textContent = linux ? "Or paste in Terminal" : "Or paste in PowerShell";
    byId("install-command").textContent = linux
      ? "curl -fsSL https://portrelay.cobanov.dev/install.sh | sh"
      : "irm https://portrelay.cobanov.dev/install.ps1 | iex";
    byId("installer-source").href = linux ? "/install.sh" : "/install.ps1";
    byId("copy-command").textContent = "Copy command";
    byId("installation-guide").firstChild.textContent = button.dataset.os === "macos" ? "Follow macOS progress " : button.dataset.os === "windows" ? "Windows setup guide " : "Linux setup guide ";
    byId("installation-guide").href = `https://github.com/cobanov/portrelay/blob/main/docs/${button.dataset.os === "macos" ? "roadmap.md" : button.dataset.os + "-alpha.md"}`;
  });
}

const copyCommand = byId("copy-command");
copyCommand.disabled = false;
copyCommand.addEventListener("click", async () => {
  const command = byId("install-command").textContent;
  try {
    await navigator.clipboard.writeText(command);
    // Do not show success for a different platform selected while permission was pending.
    if (byId("install-command").textContent === command) {
      copyCommand.textContent = "Copied!";
      byId("demo-announcement").textContent = "Installation command copied.";
    }
  } catch {
    copyCommand.textContent = "Select and copy the command above";
    byId("demo-announcement").textContent = "Clipboard unavailable. Select and copy the command above.";
  }
});

function renderFlow(device) {
  const local = view === "local";
  const active = local ? device.shared : device.connected;
  byId("relay-stage").dataset.connected = String(Boolean(active));
  byId("source-label").replaceChildren(
    document.createTextNode("PLUGGED IN "),
    element("strong", "", local ? "ON THIS COMPUTER" : "OVER THERE"),
  );
  byId("destination-label").replaceChildren(
    document.createTextNode(local ? "CHOOSE WHAT " : "READY TO USE "),
    element("strong", "", local ? "OTHERS CAN USE" : "RIGHT HERE"),
  );
  byId("source-computer").textContent = local
    ? "Work laptop"
    : peers[activePeer].name;
  byId("source-name").textContent = device.name;
  byId("source-icon").replaceChildren(icon(device.icon));
  const descriptions = {
    printer: "One printer. Wherever you work.",
    bluetooth: "One adapter. Another computer.",
    drive: "Your files, on the computer you need.",
  };
  byId("source-detail").textContent =
    descriptions[device.icon] || "Your device, between your computers.";
  byId("source-state").replaceChildren(
    element("span", "status-dot"),
    document.createTextNode(
      local
        ? active
          ? "Shared in demo"
          : "Private"
        : active
          ? "Connected in demo"
          : "Available",
    ),
  );
  byId("source-transport").textContent =
    device.type === "BT" ? "USB Bluetooth adapter" : "USB connection";
  byId("route-mode").textContent = local
    ? "PAIRED"
    : activePeer === "home"
      ? "INTERNET"
      : "LAN";
  byId("route-caption").textContent = active
    ? local
      ? "Share enabled"
      : "Connected"
    : local
      ? "Private"
      : "Click Connect";
  const lead =
    device.icon === "printer"
      ? "Your desk printer."
      : device.icon === "bluetooth"
        ? "One whole Bluetooth adapter."
        : device.icon === "drive"
          ? "Your USB drive."
          : "Your device.";
  const tail = local
    ? " Share it with a paired computer."
    : device.icon === "bluetooth"
      ? " Pair devices near the adapter, remotely."
      : " Your laptop, anywhere.";
  byId("example-caption").replaceChildren(
    element("strong", "", lead),
    document.createTextNode(tail),
  );
  for (const button of document.querySelectorAll("[data-example]")) {
    const selected =
      !local && examples[button.dataset.example].device === device.id;
    button.classList.toggle("selected", selected);
    button.setAttribute("aria-pressed", String(selected));
  }
}

for (const button of document.querySelectorAll("[data-example]")) {
  button.disabled = false;
  button.addEventListener("click", () => {
    if (!Object.hasOwn(examples, button.dataset.example)) return;
    const example = examples[button.dataset.example];
    view = "remote";
    activePeer = example.peer;
    selectedDeviceId = example.device;
    renderDevices();
    updateHint(
      button.dataset.example === "bluetooth"
        ? "Connect the adapter. Its Bluetooth range stays at Studio PC."
        : button.dataset.example === "drive"
          ? "Disk sharing needs an unmounted or offline disk. This is a simulated example."
          : "Click Connect to bring the printer over here.",
    );
  });
}
renderDevices();
