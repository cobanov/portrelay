# Project instructions

- Read README.md, docs/research.md, docs/architecture.md, and docs/roadmap.md
  before implementation. The repository is currently a research bootstrap.
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
