// Browser-only stand-in for the Rust backend, so `npm run dev` renders
// something useful without Tauri. Never used inside the app.

import defaultHud from "../../../shared/strata-hud.default.json";

const instance = {
  id: "default-26.3",
  name: "Default",
  minecraftVersion: "26.3",
  loader: "fabric",
  loaderVersion: "0.20.1",
  activeProfile: "balanced",
  memoryMb: 3072,
  extraJvmArgs: [],
};

const mods = [
  { title: "Sodium", file: "sodium-fabric-0.8.2+mc26.3.jar", version: "0.8.2", source: "core", description: "Modern rendering engine and client-side optimization mod." },
  { title: "Lithium", file: "lithium-fabric-0.17.1+mc26.3.jar", version: "0.17.1", source: "core", description: "General-purpose optimization mod." },
  { title: "ImmediatelyFast", file: "ImmediatelyFast-Fabric-1.8.0+26.3.jar", version: "1.8.0", source: "core", description: "Speeds up immediate mode rendering." },
  { title: "FerriteCore", file: "ferritecore-9.0.0-fabric.jar", version: "9.0.0", source: "core", description: "Memory usage optimizations." },
  { title: "Entity Culling", file: "entityculling-fabric-1.9.4-mc26.3.jar", version: "1.9.4", source: "core", description: "Skips rendering entities you can't see." },
  { title: "Strata HUD", file: "strata-hud-0.1.0.jar", version: "0.1.0", source: "core", description: "Keystrokes, CPS, armor status and a custom crosshair." },
  { title: "Old Zoom", file: "oldzoom-2.1.jar", version: "2.1", source: "local", description: "Zoom key from an older build.", minecraft: "~1.21.11", compatibility: "incompatible", enabled: false },
].map((m) => ({
  kind: "mod",
  enabled: true,
  size: 1_200_000,
  isDirectory: false,
  projectId: null,
  icon: null,
  modId: m.title.toLowerCase(),
  minecraft: ">=26.3",
  compatibility: "compatible",
  priority: null,
  ...m,
}));

const packs = [
  { title: "Bare Bones", description: "The simplistic look of the Minecraft trailers.", priority: 0, enabled: true },
  { title: "Clean Glass", description: "Connected glass without borders.", priority: 1, enabled: true },
  { title: "Faithful 32x", description: "Vanilla textures at double resolution.", priority: null, enabled: false },
].map((p) => ({
  kind: "resourcepack",
  file: `${p.title}.zip`,
  size: 4_000_000,
  isDirectory: false,
  source: "modrinth",
  projectId: null,
  version: null,
  icon: null,
  modId: null,
  minecraft: null,
  compatibility: "unknown",
  ...p,
}));

const hits = [
  ["Zoomify", "isXander", "A zoom mod with infinite customizability.", 21_400_000],
  ["Mod Menu", "Prospector", "Adds a mod menu to view the list of mods you have installed.", 98_000_000],
  ["Iris Shaders", "coderbot", "A modern shader pack loader for Minecraft.", 120_000_000],
  ["Better Ping Display", "Quintinity", "Shows ping numbers in the player list.", 3_100_000],
].map(([title, author, description, downloads], i) => ({
  project_id: `mock${i}`,
  slug: String(title).toLowerCase().replace(/\s/g, "-"),
  title,
  author,
  description,
  downloads,
  follows: 0,
  icon_url: null,
  categories: ["fabric"],
  date_modified: "",
}));

const profiles = [
  { id: "max-fps", name: "Max FPS", order: 1, description: "Short render distance and no visual extras.", memoryMb: 3072, jvmArgs: ["-XX:+UseG1GC", "-XX:MaxGCPauseMillis=40"], options: { renderDistance: 8, maxFps: 260 } },
  { id: "balanced", name: "Balanced", order: 2, description: "Good-looking chunks at a steady frame rate. The default.", memoryMb: 3072, jvmArgs: ["-XX:+UseG1GC", "-XX:MaxGCPauseMillis=50"], options: { renderDistance: 12, maxFps: 260 } },
  { id: "competitive", name: "Competitive PvP", order: 3, description: "Low-pause garbage collection for even frame times in fights.", memoryMb: 4096, jvmArgs: ["-XX:+UseZGC"], options: { renderDistance: 8, maxFps: 260 } },
];

const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));

export async function mockInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  await wait(120);
  const out = (v: unknown) => v as T;
  switch (cmd) {
    case "get_overview":
      return out({ instance, dataDir: "~/.local/share/Strata", irisInstalled: false });
    case "list_content":
      return out(args?.kind === "mod" ? mods : args?.kind === "resourcepack" ? packs : []);
    case "modrinth_search":
      return out({ hits, offset: 0, limit: 20, total_hits: hits.length });
    case "get_hud_config":
    case "default_hud_config":
      return out(structuredClone(defaultHud));
    case "list_profiles":
      return out(profiles);
    case "import_files":
      return out({ added: [], rejected: [] });
    case "modrinth_install":
      await wait(900);
      return out({ installed: [], satisfied: [], warnings: [] });
    default:
      return out(undefined);
  }
}
