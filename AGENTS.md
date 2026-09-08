# Project instructions

- Read README.md, docs/research.md, docs/architecture.md, and docs/roadmap.md
  before implementation. The repository contains research and a static product
  website with a simulated desktop concept; native device sharing is not built.
- Keep all public project documentation and UI text in English.
- Do not claim device or operating-system support without recorded evidence.
  Discovery, BLE GATT access, full adapter sharing, and OS-level virtual device
  attachment are different capabilities.
- Reuse established USB/IP backends. Keep privileged device operations separate
  from the UI and peer-facing agent. Never expose a raw USB/IP listener to LAN
  or internet clients as a shortcut around authenticated transport.
- Pairing, per-device authorization, exclusive ownership, revocation, and crash
  cleanup are part of the first working path, not optional post-release work.
- Do not require disabling SIP, Secure Boot, or driver-signature enforcement
  in the normal installation flow.
- Track upstream licenses and preserve notices. Commit dependency lockfiles.
- Test the layer changed. Use protocol and lifecycle tests for network/device
  code, physical hardware checks for compatibility claims, and fresh-system
  checks for installers. Keep mocked examples explicitly labeled.
- Never add assistant signatures or co-author trailers to commits or PRs.
- Website source lives in web/. Use npm run dev for a local preview and
  npm run check && npm run build before publishing. Keep this site static and
  dependency-free unless a new capability actually requires a runtime package.
- Keep example devices and sessions explicitly labeled as a preview. Do not add
  pretend download links or install commands for an unreleased desktop app.
- Production is Cloudflare Pages, project portrelay, at portrelay.cobanov.dev.
  Build with npm run build, then publish the clean committed dist/ with npm run
  deploy. Use existing Cloudflare credentials; never commit their values.
- The old private Sites preview remains recorded in .openai/hosting.json for
  history. Do not publish there instead of the user-requested Pages production.
- The product website has its own light visual identity. The user explicitly
  waived the personal site design system. Keep copy short and use the
  interactive printer, Bluetooth-adapter, and USB-drive examples to explain it.
