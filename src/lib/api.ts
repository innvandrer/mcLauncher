import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AuthPrompt,
  Instance,
  InstanceStateEvent,
  JavaInstall,
  Loader,
  LoaderVersion,
  ContentVersion,
  LogLine,
  ModEntry,
  ModUpdate,
  PublicAccount,
  ResourcePackEntry,
  ScreenshotEntry,
  SearchResponse,
  Settings,
  ShaderEntry,
  TaskProgress,
  VersionList,
  WorldEntry,
} from "./types";

export const api = {
  // Versions / loaders
  listVersions: () => invoke<VersionList>("list_minecraft_versions"),
  listFabric: (mcVersion: string) =>
    invoke<LoaderVersion[]>("list_fabric_versions", { mcVersion }),
  listQuilt: (mcVersion: string) =>
    invoke<LoaderVersion[]>("list_quilt_versions", { mcVersion }),

  // Settings
  getSettings: () => invoke<Settings>("get_settings"),
  saveSettings: (settings: Settings) => invoke<void>("save_settings", { settings }),

  // Accounts
  listAccounts: () => invoke<PublicAccount[]>("list_accounts"),
  loginMicrosoft: () => invoke<PublicAccount[]>("login_microsoft"),
  addOfflineAccount: (username: string) =>
    invoke<PublicAccount[]>("add_offline_account", { username }),
  setActiveAccount: (id: string) => invoke<PublicAccount[]>("set_active_account", { id }),
  removeAccount: (id: string) => invoke<PublicAccount[]>("remove_account", { id }),

  // Instances
  listInstances: () => invoke<Instance[]>("list_instances"),
  getInstance: (id: string) => invoke<Instance>("get_instance", { id }),
  createInstance: (args: {
    name: string;
    mcVersion: string;
    loader: Loader;
    loaderVersion?: string | null;
    icon?: string | null;
  }) => invoke<Instance>("create_instance", args),
  updateInstance: (instance: Instance) => invoke<Instance>("update_instance", { instance }),
  deleteInstance: (id: string) => invoke<void>("delete_instance", { id }),
  duplicateInstance: (id: string) => invoke<Instance>("duplicate_instance", { id }),
  openInstanceFolder: (id: string) => invoke<void>("open_instance_folder", { id }),

  // Launch
  launchInstance: (id: string) => invoke<void>("launch_instance", { id }),
  stopInstance: (id: string) => invoke<void>("stop_instance", { id }),
  runningInstances: () => invoke<string[]>("running_instances"),

  // Mods
  searchModrinth: (args: {
    query: string;
    projectType: string;
    loader?: string | null;
    gameVersion?: string | null;
    limit?: number;
    offset?: number;
  }) => invoke<SearchResponse>("search_modrinth", args),
  installMod: (args: {
    instanceId: string;
    projectId: string;
    loader?: string | null;
    gameVersion?: string | null;
  }) => invoke<string>("install_mod", args),
  listMods: (instanceId: string) => invoke<ModEntry[]>("list_mods", { instanceId }),
  setModEnabled: (instanceId: string, fileName: string, enabled: boolean) =>
    invoke<void>("set_mod_enabled", { instanceId, fileName, enabled }),
  deleteMod: (instanceId: string, fileName: string) =>
    invoke<void>("delete_mod", { instanceId, fileName }),

  // Unified content installer (mods, resource packs, shaders)
  installContent: (args: {
    instanceId: string;
    projectId: string;
    contentType: string;
    loader?: string | null;
    gameVersion?: string | null;
  }) => invoke<string>("install_content", args),

  // CurseForge (same SearchResponse/ModHit shape as Modrinth)
  searchCurseforge: (args: {
    query: string;
    contentType: string;
    loader?: string | null;
    gameVersion?: string | null;
    limit?: number;
    offset?: number;
  }) => invoke<SearchResponse>("search_curseforge", args),
  installCurseforgeContent: (args: {
    instanceId: string;
    projectId: string;
    contentType: string;
    loader?: string | null;
    gameVersion?: string | null;
  }) => invoke<string>("install_curseforge_content", args),

  // Modrinth modpacks
  installMrpack: (args: {
    instanceId: string;
    projectId: string;
    versionId?: string | null;
  }) => invoke<string>("install_mrpack", args),

  // Version lists (for the version picker)
  listModrinthVersions: (projectId: string) =>
    invoke<ContentVersion[]>("list_modrinth_versions", { projectId }),
  listCurseforgeFiles: (projectId: string) =>
    invoke<ContentVersion[]>("list_curseforge_files", { projectId }),

  // Modpacks → create a new instance from a pack
  installModrinthModpack: (args: {
    projectId: string;
    versionId?: string | null;
    name?: string | null;
    icon?: string | null;
  }) => invoke<Instance>("install_modrinth_modpack", args),
  installCurseforgeModpack: (args: {
    projectId: string;
    fileId?: string | null;
    name?: string | null;
    icon?: string | null;
  }) => invoke<Instance>("install_curseforge_modpack", args),

  // Mod updates
  checkModUpdates: (args: {
    instanceId: string;
    loader?: string | null;
    gameVersion?: string | null;
  }) => invoke<ModUpdate[]>("check_mod_updates", args),
  applyModUpdate: (instanceId: string, update: ModUpdate) =>
    invoke<void>("apply_mod_update", { instanceId, update }),

  // Export / import
  exportInstance: (id: string, dest: string) =>
    invoke<void>("export_instance", { id, dest }),
  importInstance: (src: string) => invoke<Instance>("import_instance", { src }),

  // Resource packs
  listResourcePacks: (instanceId: string) =>
    invoke<ResourcePackEntry[]>("list_resource_packs", { instanceId }),
  deleteResourcePack: (instanceId: string, fileName: string) =>
    invoke<void>("delete_resource_pack", { instanceId, fileName }),

  // Shaders
  listShaders: (instanceId: string) =>
    invoke<ShaderEntry[]>("list_shaders", { instanceId }),
  deleteShader: (instanceId: string, fileName: string) =>
    invoke<void>("delete_shader", { instanceId, fileName }),

  // Worlds
  listWorlds: (instanceId: string) =>
    invoke<WorldEntry[]>("list_worlds", { instanceId }),
  deleteWorld: (instanceId: string, name: string) =>
    invoke<void>("delete_world", { instanceId, name }),
  openWorldFolder: (instanceId: string, name: string) =>
    invoke<void>("open_world_folder", { instanceId, name }),

  // Screenshots
  listScreenshots: (instanceId: string) =>
    invoke<ScreenshotEntry[]>("list_screenshots", { instanceId }),
  openScreenshot: (instanceId: string, fileName: string) =>
    invoke<void>("open_screenshot", { instanceId, fileName }),

  // Java
  detectJava: () => invoke<JavaInstall[]>("detect_java"),

  // Project body / descriptions (for detail pages)
  getModrinthProjectBody: (projectId: string) =>
    invoke<string>("get_modrinth_project_body", { projectId }),
  getCurseforgeDescription: (projectId: string) =>
    invoke<string>("get_curseforge_description", { projectId }),

  // Version-specific installs (for the version picker)
  installContentVersion: (args: {
    instanceId: string;
    projectId: string;
    versionId: string;
    contentType: string;
  }) => invoke<string>("install_content_version", args),
  installCurseforgeFile: (args: {
    instanceId: string;
    projectId: string;
    fileId: string;
    contentType: string;
  }) => invoke<string>("install_curseforge_file", args),

  // Open a URL in the system browser
  openUrl: (url: string) => invoke<void>("open_url", { url }),
};

export const events = {
  onTaskProgress: (cb: (p: TaskProgress) => void): Promise<UnlistenFn> =>
    listen<TaskProgress>("task://progress", (e) => cb(e.payload)),
  onLog: (cb: (l: LogLine) => void): Promise<UnlistenFn> =>
    listen<LogLine>("instance://log", (e) => cb(e.payload)),
  onInstanceState: (cb: (s: InstanceStateEvent) => void): Promise<UnlistenFn> =>
    listen<InstanceStateEvent>("instance://state", (e) => cb(e.payload)),
  onAuthPrompt: (cb: (p: AuthPrompt) => void): Promise<UnlistenFn> =>
    listen<AuthPrompt>("auth://prompt", (e) => cb(e.payload)),
};

export function errMessage(e: unknown): string {
  if (typeof e === "string") return e;
  if (e && typeof e === "object" && "message" in e) return String((e as any).message);
  return String(e);
}
