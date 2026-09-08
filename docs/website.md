# Product website

Production: **[portrelay.cobanov.dev](https://portrelay.cobanov.dev)**, hosted on
Cloudflare Pages. The desktop agent, real device sharing, and installers remain
unreleased. This website is a working interface concept.

## Design

The page uses its own light identity: white surfaces, blue actions, readable
Inter and Manrope typography, and a diagram with two endpoints. The user
explicitly requested a cleaner design independent of the personal site theme.

Instead of long feature descriptions, three examples show the intended flow:

- A printer shared from Studio PC to Work laptop.
- A whole USB Bluetooth adapter lent to another computer.
- A USB drive accessed from Home server over a simulated internet connection.

Connecting a device updates its status, the source card, the receiver list,
connection count, and animated path. Reduced-motion preferences disable the
animation. A compact three-step installation outline and platform selector sit
below the example. No installer or support claim is fabricated.

## Interaction boundaries

All state stays in browser memory and resets on reload. No device APIs, local
agents, pairing secrets, or real network sessions are used. Busy devices remain
unavailable, local sharing can be toggled, and the example local keyboard stays
protected. Linux, Windows, and macOS statuses reflect the development plan.

The Bluetooth example lends a whole adapter; it does not imply general
virtualization of individual Bluetooth peripherals. Device compatibility still
requires actual hardware evidence, separately from this visual demonstration.

The page remains readable without JavaScript, with demo controls disabled. It
includes keyboard focus restoration, status announcements, responsive layouts,
and reduced-motion handling. Browser visual and interaction QA has not been
performed in this task; syntax, formatting, build, assets, and deployed HTTP
responses are checked separately.

## Build and deployment

`npm run dev` serves `web/` on loopback. `npm run check` checks JavaScript syntax.
`npm run build` verifies local links and asset references, then copies `web/`
into `dist/`. There are no runtime JavaScript packages. Fonts are loaded from
Google Fonts with system fallbacks. There is no signup service. Cloudflare's
current zone configuration injects its Web Analytics beacon into production HTML.

Cloudflare Pages configuration is in `wrangler.json`:

- Project: `portrelay`
- Production branch: `main`
- Static output: `dist/`
- Custom domain: `portrelay.cobanov.dev`
- DNS: proxied CNAME to `portrelay.pages.dev`

After building and committing, run `npm run deploy`. The command uses pinned
Wrangler 4.105.0 and existing `CLOUDFLARE_API_TOKEN` / `CLOUDFLARE_ACCOUNT_ID`
environment credentials. Pages configuration does not accept `account_id`.
Never write token values into the repository. Future source changes require a
new deploy; this is a direct-upload Pages project, not a Git-integrated build.

The `_headers` file sets response headers and disables browser device access.
Canonical metadata, robots.txt, and sitemap.xml point to the custom domain.
The output can be deployed to other static hosts without a platform runtime.

The previous private preview at `portrelay.cobanovdev.chatgpt.site` and its
`.openai/hosting.json` project identifier are retained as history. They are not
the production deployment target and are not synchronized by Pages deployment.

## Production verification

On 2026-09-08, production deployment `068648ba-2e7f-4091-9967-6a577df31757`
published source commit `0ef586391fbeb0e74850c916b0c6fa9aae488dbd`. Cloudflare
reports the custom domain and certificate validation as active. HTTPS requests
using a browser user agent return 200 for the page and all five referenced or
discovery assets. CSS, JavaScript, favicon, robots, and sitemap match the build
byte for byte. HTML differs only by Cloudflare's injected analytics script.
The published response headers are present. Default Python user-agent requests
receive 403 from the existing edge configuration; browser-user-agent HTTP checks
pass. These checks do not constitute browser rendering or interaction QA.
