// Typed wrappers around the Rust commands in src-tauri/src/lib.rs.
// Outside Tauri (plain `npm run dev` in a browser) calls go to a small mock
// so the UI can be worked on without the backend.

import { invoke as tauriInvoke, isTauri } from "@tauri-apps/api/core";

export type Kind = "mod" | "resourcepack" | "shader";
export type Source = "core" | "modrinth" | "local";
export type Compat = "compatible" | "incompatible" | "unknown";
export type Sort = "relevance" | "downloads" | "follows" | "newest" | "updated";

export interface ContentItem {
  kind: Kind;
  file: string;
  enabled: boolean;
  size: number;
  isDirectory: boolean;
  source: Source;
  projectId: string | null;
  title: string;
  version: string | null;
  description: string | null;
  icon: string | null;
  modId: string | null;
  minecraft: string | null;
  compatibility: Compat;
  priority: number | null;
}

export interface Instance {
  id: string;
  name: string;
  minecraftVersion: string;
  loader: string;
  loaderVersion: string;
  activeProfile: string;
  memoryMb: number;
  extraJvmArgs: string[];
}

export interface Overview {
  instance: Instance;
  dataDir: string;
  irisInstalled: boolean;
}

export interface SearchHit {
  project_id: string;
  slug: string;
  title: string;
  description: string;
  author: string;
  icon_url: string | null;
  downloads: number;
  follows: number;
  categories: string[];
  date_modified: string;
}

export interface SearchResults {
  hits: SearchHit[];
  offset: number;
  limit: number;
  total_hits: number;
}

export interface InstallReport {
  installed: {
    projectId: string;
    title: string;
    file: string;
    versionNumber: string;
    replaced: string | null;
    dependency: boolean;
  }[];
  satisfied: string[];
  warnings: string[];
}

export interface ImportReport {
  added: { file: string; replaced: string | null; warning: string | null }[];
  rejected: { file: string; reason: string }[];
}

export interface Progress {
  requestId: string;
  projectId: string;
  title: string;
  stage: "resolving" | "downloading" | "done" | "failed";
  received: number;
  total: number;
}

export interface Profile {
  id: string;
  name: string;
  order: number;
  description: string;
  memoryMb: number;
  jvmArgs: string[];
  options: Record<string, number | boolean>;
}

export type CrosshairStyle = "plus" | "t" | "x" | "circle" | "dot";

export interface CrosshairConfig {
  enabled: boolean;
  style: CrosshairStyle;
  length: number;
  gap: number;
  thickness: number;
  dot: boolean;
  dotSize: number;
  outline: boolean;
  color: string;
  outlineColor: string;
  colorOnTarget: boolean;
  targetColor: string;
  cooldownIndicator: boolean;
}

export interface ModuleConfig {
  enabled: boolean;
  x: number;
  y: number;
  scale: number;
  [extra: string]: unknown;
}

export interface HudConfig {
  version: number;
  theme: Record<string, string | boolean>;
  crosshair: CrosshairConfig;
  modules: Record<string, ModuleConfig>;
}

export type AccountKind = "microsoft" | "test";

export interface Account {
  uuid: string;
  name: string;
  kind: AccountKind;
  active: boolean;
  skin: string | null;
  skinVariant: "classic" | "slim" | string;
}

export interface AccountsView {
  accounts: Account[];
  clientIdSet: boolean;
  testAccountsAllowed: boolean;
}

export interface DeviceCode {
  userCode: string;
  verificationUri: string;
  expiresIn: number;
}

export interface AuthStatus {
  state: "waiting" | "signing-in" | "done" | "failed" | "cancelled";
  message: string | null;
}

export interface VersionOption {
  id: string;
  installed: boolean;
}

export interface LaunchProgress {
  stage: "account" | "metadata" | "downloading" | "java" | "starting" | "running";
  label: string;
  done: number;
  total: number;
}

export interface GameStatus {
  running: boolean;
  launching: boolean;
  log: string[];
}

export interface GameExit {
  code: number | null;
  crashed: boolean;
  seconds: number;
}

export const inTauri = isTauri();

async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (inTauri) return tauriInvoke<T>(cmd, args);
  // Loaded lazily so the mock never ships inside the app's main bundle.
  const { mockInvoke } = await import("./mock");
  return mockInvoke<T>(cmd, args);
}

export const api = {
  overview: () => invoke<Overview>("get_overview"),
  list: (kind: Kind) => invoke<ContentItem[]>("list_content", { kind }),
  setModEnabled: (file: string, enabled: boolean) => invoke<void>("set_mod_enabled", { file, enabled }),
  setResourcePacks: (enabled: string[]) => invoke<void>("set_resource_packs", { enabled }),
  setActiveShader: (file: string | null) => invoke<void>("set_active_shader", { file }),
  remove: (kind: Kind, file: string) => invoke<void>("remove_content", { kind, file }),
  openFolder: (kind: Kind) => invoke<void>("open_content_folder", { kind }),
  importFiles: (kind: Kind, paths: string[]) => invoke<ImportReport>("import_files", { kind, paths }),
  search: (query: { kind: Kind; query: string; sort: Sort; offset: number; limit: number }) =>
    invoke<SearchResults>("modrinth_search", { query }),
  install: (kind: Kind, project: string) => invoke<InstallReport>("modrinth_install", { kind, project }),
  hud: () => invoke<HudConfig>("get_hud_config"),
  saveHud: (config: HudConfig) => invoke<void>("save_hud_config", { config }),
  defaultHud: () => invoke<HudConfig>("default_hud_config"),
  profiles: () => invoke<Profile[]>("list_profiles"),
  setProfile: (profile: string) => invoke<void>("set_active_profile", { profile }),

  accounts: () => invoke<AccountsView>("get_accounts"),
  setClientId: (id: string) => invoke<void>("set_client_id", { id }),
  beginSignIn: () => invoke<DeviceCode>("auth_begin"),
  cancelSignIn: () => invoke<void>("auth_cancel"),
  openSignInPage: () => invoke<void>("auth_open_link"),
  selectAccount: (uuid: string) => invoke<void>("select_account", { uuid }),
  signOut: (uuid: string) => invoke<void>("sign_out", { uuid }),
  addTestAccount: (name: string) => invoke<void>("add_test_account", { name }),

  versions: () => invoke<VersionOption[]>("list_versions"),
  setVersion: (version: string) => invoke<Instance>("set_version", { version }),
  launch: () => invoke<void>("launch_game"),
  stop: () => invoke<void>("stop_game"),
  gameStatus: () => invoke<GameStatus>("game_status"),
};

/** Error messages from Rust arrive as plain strings. */
export function errorText(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return "Something went wrong.";
}
