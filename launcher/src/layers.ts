import type { Kind } from "./lib/api";

export type LayerId = "mods" | "packs" | "shaders" | "hud" | "profiles";

export interface Layer {
  id: LayerId;
  name: string;
  /** Height of this layer's band in the sidebar core sample. Deliberately uneven. */
  band: number;
  tone: string;
  kind?: Kind;
  blurb: string;
  dropHint?: string;
}

export const LAYERS: Layer[] = [
  {
    id: "mods",
    name: "Mods",
    band: 62,
    tone: "var(--shale-blue)",
    kind: "mod",
    blurb: "Fabric mods for this instance. Turn one off to test without it; Strata keeps the file.",
    dropHint: "Fabric .jar files",
  },
  {
    id: "packs",
    name: "Resource packs",
    band: 46,
    tone: "var(--sandstone)",
    kind: "resourcepack",
    blurb: "Packs at the top of the list win when two change the same texture.",
    dropHint: ".zip packs or pack folders",
  },
  {
    id: "shaders",
    name: "Shaders",
    band: 34,
    tone: "var(--oxide)",
    kind: "shader",
    blurb: "Shader packs run through Iris. Pick one, or none for the best frame rate.",
    dropHint: ".zip shader packs",
  },
  {
    id: "hud",
    name: "HUD",
    band: 54,
    tone: "var(--lichen)",
    blurb: "Your crosshair and which overlays show in game. Press Right Shift in game to move them.",
  },
  {
    id: "profiles",
    name: "Profiles",
    band: 40,
    tone: "var(--tuff)",
    blurb: "Java and video settings Strata applies when the game starts.",
  },
];
