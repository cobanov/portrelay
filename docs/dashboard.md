# The PortRelay dashboard

Available in alpha.9 / Mac preview.4. The local app keeps its light theme and
uses a compact sidebar. On narrow screens, open the menu in the top left.

- **Computers:** see this computer and your registered peers. **USB devices**
  opens a peer's shared devices; **Control** opens keyboard and mouse setup.
- **USB devices:** choose a computer, then **Share from here** or
  **Use a remote device**. Handoff and disk-ejection warnings still require
  confirmation. Unavailable devices remain visible under the protected list.
- **Keyboard & mouse:** choose the receiving Linux computer and start control.
  Press **Esc** to return. Linux receiving setup is on the same screen; allow
  individual controllers under **Computers → Manage**.
- **Connections:** see active sessions, disconnect, and inspect recovery errors.
- **Settings:** manage GitHub sign-in, rename this computer, enable USB support,
  or use advanced offline pairing. The browser remembers the last open screen.

**Add computer** explains installation and GitHub sign-in in three short steps,
including the headless login command. Your account never automatically grants
USB or input permissions. Device transport and platform limits are unchanged.

## UI development

`node tests/ui-preview.mjs` serves only on `127.0.0.1:4175`. Scenarios `mac`,
`linux`, `first-run`, and `empty` use explicitly labeled example data. No real
account, USB or input operation is performed. This fixture server is not part
of the desktop package or public website.
