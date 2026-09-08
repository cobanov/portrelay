# Product website

The first website presents the intended installation flow and a desktop UI
concept. It does not install PortRelay or share hardware. The desktop agent and
native installer milestones remain open in the [roadmap](roadmap.md).

## Design

The reference is the approachable setup and desktop product presentation of
[Tailscale](https://tailscale.com/), adapted to PortRelay's own identity. No
Tailscale branding, screenshots, copy, or product claims are reused.

The layout pairs a three-step explanation with an interactive desktop window.
A dark background and a single connection-status green accent follow the
personal site design language. Typography uses Geist and JetBrains Mono with
system fallbacks; fonts are loaded from Google Fonts. All public copy is English
under the repository's language convention. There is no analytics script,
tracking cookie, signup form, or server runtime.

## Interaction boundaries

- Remote device buttons update only the example session in memory.
- A busy example webcam cannot be claimed. A protected local keyboard cannot be
  shared. These demonstrate intended product behavior, not OS enforcement.
- A dedicated Bluetooth adapter is shown as one whole device. The site never
  implies that every Bluetooth peripheral can be forwarded independently.
- Paired-computer selection demonstrates LAN and relayed internet contexts. It
  does not establish network connections or measure connection performance.
- Local sharing can be turned on and off. Reloading resets every example.
- The OS selector explains Linux-first development, later Windows work, and the
  unresolved macOS USB permissions. There are no fake download buttons.
- Without JavaScript, all product information remains readable and the example
  controls remain disabled.

## Build and hosting

The Node scripts require no third-party dependencies. `npm run check` checks
JavaScript syntax. `npm run build` validates local asset references, in-page
anchors and duplicate IDs, then copies only `web/` to `dist/`. The development
server binds to loopback and serves only the website directory.

The site can be served by any static host. `.openai/hosting.json` records the
existing Sites project and the `dist` output directory. The initial Sites
deployment is an owner-only preview, not a public product launch. Device
functionality and public release availability must remain separate from the
website's deployment status.

The page includes semantic landmarks, keyboard focus styles, a skip link,
announced demo actions, responsive layouts, and reduced-motion handling. Browser
interaction and visual QA have not been run in this initial website task.
