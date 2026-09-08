# Contributing to PortRelay

The repository contains the Windows and Linux application, separate privileged
device helpers, local control UI, research, and a static product demo. Start with
[the roadmap](docs/roadmap.md) and [validation record](docs/validation.md).
Build and check the Rust workspace with `cargo build --locked`,
`cargo test --locked --workspace`, and
`cargo clippy --locked --workspace --all-targets -- -D warnings`.
Website commands are in the [README](README.md#website-development).
Windows packaging and the separate GPL device service are described in
[ADR 0003](docs/adr-0003-windows-alpha.md) and the
[Windows source notices](windows/THIRD-PARTY-NOTICES.md).

Keep changes focused on a documented milestone. For backend changes, include the
operating system, kernel or driver version, device identifiers, and what a real
application could do with the remote device. Redact serial numbers, pairing
invitations, private keys, and personal device names from public reports.

Mocked device lists, a successful build, and a successful network connection do
not establish hardware support. Distinguish automated protocol tests, virtual
hardware tests, and physical device tests in every compatibility claim.

The project reuses drivers instead of starting a new kernel driver project. Do
not copy upstream code without checking its license and preserving required
notices. Record third-party versions, licenses, and source locations when
dependencies are introduced. Keep application lockfiles in version control.

Report security problems using [the security policy](SECURITY.md). Ordinary bugs
and feature proposals can be submitted through GitHub issues. Pull requests
should explain the behavior change, its reason, and relevant verification.
