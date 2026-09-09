// Loopback-only UI fixtures. No real account, peer, USB or input operations.
// Run: node tests/ui-preview.mjs; open ?scenario=mac, linux, first-run or empty.
import { createServer } from "node:http";
import { readFile, appendFile } from "node:fs/promises";
const root = new URL("../app-ui/", import.meta.url);
const scenarios = new Map(),
  operations = [];
const device = (id, name, kind, extra = {}) => ({
  id,
  name: `${name} (example)`,
  kind,
  generation: `gen-${id}`,
  blocked: null,
  busy: false,
  risks: [],
  ...extra,
});
function fixture(name) {
  const signed = name !== "first-run",
    mac = name === "mac";
  return {
    name: "Design preview",
    id: "local-example",
    version: "0.1.0-alpha.9 · UI fixture",
    platform: mac ? "macos" : "linux",
    network: "Direct or internet relay",
    helper_ready: name !== "first-run",
    helper_error: null,
    setup_available: !mac,
    setup: {},
    capabilities: {
      usb_export: true,
      usb_import: !mac,
      import_reason: "Receiving USB on Mac is not available yet.",
    },
    input: {
      receive_supported: !mac,
      ready: signed,
      setup_available: !mac,
      setup: {},
      controllers: signed ? ["spark-example"] : [],
    },
    account: {
      status: signed ? "connected" : "signed_out",
      identity: signed
        ? { name: "example-account", id: "github:example" }
        : null,
      error: null,
    },
    peers:
      signed && name !== "empty"
        ? {
            "spark-example": {
              name: "Spark (example)",
              approved: true,
              online: true,
              account_id: "github:example",
            },
            "pi-example": {
              name: "Raspberry Pi (example)",
              approved: true,
              online: false,
              account_id: "github:example",
            },
          }
        : {},
    devices: [
      device("serial", "USB serial adapter", "usb"),
      device("disk", "External SSD", "storage", { risks: ["storage"] }),
      device("hub", "USB hub", "hub", {
        blocked: "The hub itself stays local.",
      }),
      device("bluetooth", "Bluetooth adapter", "bluetooth", {
        parent_hub: "hub",
        risks: ["bluetooth"],
      }),
      device("keyboard", "Keyboard", "input", {
        parent_hub: "hub",
        risks: ["input"],
      }),
      device("system", "System device", "usb", {
        blocked: "Used by the operating system.",
      }),
    ],
    grants: {},
    sessions: [],
    history: [],
  };
}
const files = new Set([
  "index.html",
  "app.js",
  "style.css",
  "input.js",
  "input-events.js",
]);
const types = { html: "text/html", js: "text/javascript", css: "text/css" };
createServer(async (req, res) => {
  try {
    const url = new URL(req.url, "http://127.0.0.1");
    const requested = url.searchParams.get("scenario");
    const name = ["mac", "linux", "first-run", "empty"].includes(requested)
      ? requested
      : req.headers.cookie?.match(/(?:^|; )scenario=([\w-]+)/)?.[1] || "mac";
    if (!scenarios.has(name) || url.searchParams.has("reset"))
      scenarios.set(name, fixture(name));
    const state = scenarios.get(name);
    res.setHeader("cache-control", "no-store");
    res.setHeader(
      "content-security-policy",
      "default-src 'self'; script-src 'self'; style-src 'self'; connect-src 'self'; img-src 'self' data:; frame-ancestors 'none'; base-uri 'none'; form-action 'none'",
    );
    function json(value, status = 200) {
      res.writeHead(status, { "content-type": "application/json" });
      res.end(JSON.stringify(value));
    }
    if (url.pathname === "/api/state") return json(state);
    if (url.pathname === "/fixture-operations") return json(operations);
    if (url.pathname === "/api/action") {
      let body = "";
      for await (const chunk of req) {
        body += chunk;
        if (body.length > 16384)
          return json({ error: "Request too large" }, 413);
      }
      const action = JSON.parse(body);
      operations.push({ scenario: name, ...action });
      if (process.env.UI_AUDIT_FILE)
        await appendFile(
          process.env.UI_AUDIT_FILE,
          JSON.stringify({ scenario: name, ...action }) + "\n",
        );
      switch (action.op) {
        case "remote":
          return action.peer === "pi-example"
            ? json({ error: "Example computer is offline." }, 400)
            : json({
                devices: [
                  device("remote-serial", "Remote USB serial adapter", "usb"),
                  device("remote-disk", "Remote SSD", "storage", {
                    risks: ["storage"],
                  }),
                  device("occupied", "Camera", "usb", { busy: true }),
                ],
              });
        case "rename":
          state.name = action.name;
          return json({});
        case "share":
          state.grants[action.device] = {
            generation: action.generation,
            peers: [action.peer],
          };
          return json({});
        case "unshare":
          delete state.grants[action.device];
          return json({});
        case "connect":
          state.sessions.push({
            id: `session-${action.device}`,
            device: action.device,
            peer: action.peer,
            state: "connected",
            direction: "incoming",
            metadata: device(action.device, "Remote device", "usb"),
          });
          return json({ message: "USB transport attached (example only)." });
        case "disconnect":
          state.sessions = state.sessions.filter(
            (s) => s.id !== action.session,
          );
          return json({ message: "Example device returned." });
        case "revoke":
          delete state.peers[action.peer];
          return json({});
        case "approve":
          state.peers[action.peer].approved = true;
          return json({});
        case "input_allow":
          state.input.controllers = action.allowed
            ? [...state.input.controllers, action.peer]
            : state.input.controllers.filter((p) => p !== action.peer);
          return json({});
        case "setup_usb":
          state.helper_ready = true;
          return json({});
        case "setup_input":
          state.input.ready = true;
          return json({});
        case "account_logout":
          state.account = { status: "signed_out", identity: null };
          state.peers = {};
          return json({});
        case "account_login":
          state.account = {
            status: "pending",
            identity: null,
            authorization_url: "http://127.0.0.1:4175/fixture-login",
          };
          return json({ authorization_url: state.account.authorization_url });
        case "invite":
          return json({
            invitation: "EXAMPLE-INVITATION-NOT-VALID",
            expires: Math.floor(Date.now() / 1000) + 600,
          });
        case "pair":
          state.peers["direct-example"] = {
            name: "Paired computer (example)",
            approved: true,
          };
          return json({});
        case "input_start":
          return json(
            { error: "Design preview only. No input is sent to any computer." },
            400,
          );
        default:
          return json({ error: "Unsupported fixture action" }, 400);
      }
    }
    if (url.pathname === "/fixture-login") {
      res.setHeader("content-type", "text/plain");
      res.end("UI fixture only. No GitHub sign-in is performed.");
      return;
    }
    const file = url.pathname === "/" ? "index.html" : url.pathname.slice(1);
    if (!files.has(file)) {
      res.writeHead(404).end();
      return;
    }
    if (file === "index.html")
      res.setHeader("set-cookie", `scenario=${name}; Path=/; SameSite=Strict`);
    const body = await readFile(new URL(file, root));
    res.setHeader(
      "content-type",
      types[file.split(".").at(-1)] + "; charset=utf-8",
    );
    res.end(body);
  } catch {
    res.writeHead(500).end("Fixture error");
  }
}).listen(4175, "127.0.0.1", () =>
  console.log("UI fixture: http://127.0.0.1:4175/?scenario=mac"),
);
