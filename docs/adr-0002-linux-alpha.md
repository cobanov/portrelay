# ADR 0002: A small working Linux application before a native shell

Date: 2026-09-08

The first implementation uses Rust 1.97.0, iroh 1.1.0, a separate privileged
Linux helper, and an embedded local HTML/CSS/JavaScript control window served by
Axum. The same executable includes a CLI for automation and headless use.

Tauri remains a future packaging option. A local browser window lets the first
Linux installer ship without a WebKit runtime and keeps all device operations
behind the same agent API. The interface displays actual state and calls the
agent; it is not the website demo repackaged as an application.

The Linux helper directly supplies connected socket descriptors to the existing
USB/IP kernel export and virtual-host-controller interfaces. Distribution
`usbip bind` / `usbip unbind` handle binding lifecycle. This avoids exposing a
raw USB/IP daemon listener. Device data is streamed through private local IPC
and a reliable encrypted QUIC stream with bounded buffering.

LAN/direct mode has no public services. A configurable iroh relay supports
internet routes; relay-only mode removes direct IP transports for explicit
fallback testing. No shared production relay is deployed by this change.

This implementation does not make Windows/macOS virtual USB drivers available,
provide individual BLE profile proxies, or replace physical compatibility tests.
