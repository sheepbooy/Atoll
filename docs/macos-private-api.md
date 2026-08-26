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

## MediaRemote (Now Playing)

The Now Playing card reads media metadata via the MediaRemote private framework,
but on macOS 26 third-party (non-arm64e) apps cannot read MediaRemote data
directly. Atoll bundles the BSD-3-licensed `MediaRemoteAdapter.framework`
(precompiled universal binary including arm64e) under
`src-tauri/resources/media/`, invoked via the `mediaremote-adapter.pl` script.

**How it works:**
- `fetch_now_playing` spawns `/usr/bin/perl mediaremote-adapter.pl <framework> get`
  and parses the JSON stdout (title, artist, album, duration, elapsedTime,
  playing, artworkData base64, bundleIdentifier).
- `send_media_command` invokes the script with `send <MRCommand>`.

**Failure risk:**
- Apple may break the arm64e adapter in a future macOS — the card degrades
  silently (adapter exits non-zero → `None` → card does not render).
- `/usr/bin/perl` must exist (stock macOS ships it).
- `MRMediaRemoteSendCommand` may require the app to be frontmost or have
  accessibility permissions on some macOS versions.

**Degradation path:**
- If the script or framework is missing, commands return `None`/`false` and
  the card does not render (no crash).
- No CGEvent media-key fallback is implemented; add if needed.
