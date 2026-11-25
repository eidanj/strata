# Strata

A lightweight Minecraft PvP launcher (Tauri 2 + React/TypeScript, Rust backend) and its in-game HUD mod (Fabric, Minecraft 26.3).

```
strata/
├─ launcher/            Tauri app
│  ├─ src/              React UI (presentation only)
│  └─ src-tauri/src/    Rust: files, Modrinth, drag-and-drop, HUD config
├─ hud-mod/             Fabric mod: crosshair, Right Shift editor, PvP overlays
├─ shared/              strata-hud.default.json, used by both of the above
└─ design/              icon source + the script that draws it
```

## What works in this starter

| Area | Status |
|---|---|
| App shell, theme, five sections | Done |
| Mods / Resource packs / Shaders lists (enable, disable, remove, pack priority, active shader) | Done |
| Drag-and-drop and "Add files" import, with validation and helpful rejections | Done, tested |
| Modrinth search filtered to 26.3 + Fabric, install with required dependencies, SHA-512 check, progress | Done |
| Mod version compatibility check (reads `fabric.mod.json`) | Done, tested |
| HUD tab: crosshair editor with live preview, overlay toggles and size | Done |
| Profiles: list and select | Done (applied at launch in Phase 3) |
| Microsoft sign-in, game download, launching | Phase 1, not started |
| Bundled performance core auto-install | Phase 3, not started |

## Run the launcher

Prerequisites: Rust 1.85+, Node 20+, and Tauri's OS prerequisites
(Windows: WebView2, already on Windows 11; macOS: Xcode command line tools;
Linux: `libwebkit2gtk-4.1-dev` and friends, see https://v2.tauri.app/start/prerequisites/).

```bash
cd launcher
npm install
npm run tauri dev      # app with hot reload
npm run tauri build    # installer in src-tauri/target/release/bundle
```

`npm run dev` on its own opens the UI in a browser with mock data (no Rust needed),
and `?tab=hud` (or `packs`, `shaders`, `profiles`) opens a section directly.

Tests: `cd launcher/src-tauri && cargo test` covers the version-range matcher and an
end-to-end drag-and-drop run (Fabric/Forge detection, duplicate drops, updates replacing
older jars, pack order written to `options.txt`, shader selection).

## Build the HUD mod

Needs JDK 25.

```bash
cd hud-mod
./gradlew build        # -> build/libs/strata-hud-0.1.0.jar
```

Drop the jar onto Strata's Mods tab (or into the instance's `mods/` folder) along with
Fabric API. In game:

- **Right Shift** opens the layout editor. Drag to move, scroll to resize, right-click to
  show or hide, arrow keys nudge (Shift for 5px). Overlays snap to screen edges, the
  centre and each other.
- **Crosshair** (button in the editor) opens the crosshair editor with a 5x live preview.
- Overlays: FPS, CPS, Ping, Keystrokes (with CPS under the mouse buttons), Armor status,
  Held items (durability or total count carried), Toggle sprint, Coordinates.

**Important:** I couldn't compile the mod against real 26.3 in my environment (Mojang's
and Fabric's servers were unreachable). Every Fabric API call and the trickier vanilla
names (`GuiGraphicsExtractor.text/item/itemDecorations`, `extractRenderState`,
`extractBackground`, `MouseButtonEvent`, `gui.setScreen`, SDL scancodes) were checked
against Fabric API's and JEI's 26.3 source, and the mod compiles against stand-ins for
that API. If the first real build reports a missing method, it will almost certainly be in
`render/Draw.java` or `render/Game.java`, which hold every version-sensitive call.

The crosshair geometry in `CrosshairRenderer.java` and `launcher/src/lib/crosshair.ts`
was checked to produce identical pixels across 360 configurations. Change them together.

## Where things live on disk

`<OS data dir>/Strata` (`%APPDATA%\Strata`, `~/Library/Application Support/Strata`,
`~/.local/share/Strata`), or `$STRATA_HOME`:

```
Strata/
├─ strata.json                 launcher settings
├─ meta/  core/                shared game files and the bundled core (Phase 1/3)
├─ profiles/*.json             performance profiles (seeded on first run, never overwritten)
└─ instances/default-26.3/
   ├─ instance.json            version, loader, active profile, memory
   ├─ content.lock.json        where each mod/pack came from: core, modrinth or local
   └─ minecraft/               the game directory, isolated from .minecraft
      ├─ mods/  resourcepacks/  shaderpacks/
      ├─ options.txt           pack order is written here (resourcePacks:[...])
      └─ config/strata-hud.json  shared by the launcher's HUD tab and the mod
```

The folder is the source of truth for what exists; the lock file only adds where it came
from. Disabled mods are renamed to `*.jar.disabled`, which Fabric ignores.

## Notes

- **Drag-and-drop**: Tauri's native drop event (real file paths) goes to whichever tab is
  open. On Windows HTML5 drag events don't fire inside the webview while native drops are
  on, which is why pack reordering uses buttons rather than drag handles.
- **Modrinth**: set the contact in `USER_AGENT` (`src-tauri/src/modrinth.rs`) before
  shipping. The limit is 300 requests a minute per IP; the client retries once on 429.
  Dependencies already present (by Modrinth project, or by a jar whose mod id matches the
  project slug) are skipped.
- **Toggle sprint** switches on vanilla's own "Sprint: Toggle" option, so it behaves like
  the game's toggle and is fine on any server.
- **Profiles**: `-XX:+UseCompactObjectHeaders` and generational Shenandoah are standard
  in Java 25. Treat the flags as starting points and benchmark frame times in Phase 3.

## Next: Phase 1

1. Register an Azure app (personal Microsoft accounts, public client flows on) and apply
   for Minecraft API access at https://aka.ms/mce-reviewappid. Approval takes time, so
   start now.
2. Rust `auth.rs`: device-code flow → Xbox Live → XSTS → `login_with_xbox` → profile.
   Keep the refresh token in the OS keychain (`keyring` crate).
3. Rust `launch.rs`: version manifest → 26.3 JSON → libraries, assets and natives (26.3
   uses SDL, not GLFW: follow the JSON) → Fabric profile from `meta.fabricmc.net` → build
   the command from `arguments.jvm`/`arguments.game` → spawn Java 25 with
   `--gameDir instances/<id>/minecraft` → stream logs → hide the launcher.
