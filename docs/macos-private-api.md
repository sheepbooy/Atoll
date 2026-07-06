# macOS Private API Note

Atoll currently keeps `app.macOSPrivateApi` enabled in `src-tauri/tauri.conf.json`
because the floating island uses NSPanel-style window behavior that Tauri exposes
through this setting.

Known tradeoffs:

- Mac App Store distribution is not supported while private APIs are enabled.
- Future macOS releases may change private window behavior without notice.
- Release validation should include a manual smoke test for floating, focus, and
  menu-bar positioning behavior on the oldest supported macOS version.

If public Tauri APIs gain equivalent always-on-top panel behavior, prefer migrating
to that path and disabling `macOSPrivateApi`.
